from __future__ import annotations

import argparse
import hashlib
import os
import re
import webbrowser
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import yaml
from pylatexenc.latex2text import LatexNodes2Text
from rich.console import Group
from rich.markdown import Markdown as RichMarkdown
from rich.text import Text
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical, VerticalScroll
from textual.widgets import Footer, Header, Input, ListItem, ListView, Static
from textual_image.renderable import Image as RichImage


IMAGE_PATTERN = re.compile(r"!\[(?P<alt>[^\]]*)\]\((?P<href>[^)]+)\)")
INLINE_MATH_PATTERN = re.compile(r"(?<!\$)\$(?!\$)(.+?)(?<!\$)\$(?!\$)")
BLOCK_PATTERN = re.compile(
    r"(?ms)"
    r"(?P<fence>^```(?:latex|math)\n(?P<fence_body>.*?)\n```[ \t]*\n?)"
    r"|(?P<display>^\$\$\n?(?P<display_body>.*?)\n?\$\$[ \t]*\n?)"
    r"|(?P<image>^(?P<image_line>!\[(?P<image_alt>[^\]]*)\]\((?P<image_href>[^)]+)\))[ \t]*\n?)"
)
LATEX_TO_TEXT = LatexNodes2Text()


@dataclass(slots=True)
class NoteImage:
    alt: str
    href: str
    path: Path | None


@dataclass(slots=True)
class Note:
    path: Path
    title: str
    tags: list[str]
    body: str
    meta: dict[str, Any] = field(default_factory=dict)
    images: list[NoteImage] = field(default_factory=list)

class Vault:
    def __init__(self, root: Path) -> None:
        self.root = root
        self.notes_dir = root / "notes"

    def load_notes(self) -> list[Note]:
        notes: list[Note] = []
        if not self.notes_dir.exists():
            return notes

        for path in sorted(self.notes_dir.rglob("*.md")):
            notes.append(self._load_note(path))

        notes.sort(key=lambda note: note.path.name.casefold())
        return notes

    def _load_note(self, path: Path) -> Note:
        raw_text = path.read_text(encoding="utf-8")
        meta, body = self._split_frontmatter(raw_text)
        tags = self._normalize_tags(meta.get("tags", []))
        title = str(meta.get("title") or path.stem.replace("-", " ").replace("_", " "))

        images: list[NoteImage] = []
        for match in IMAGE_PATTERN.finditer(body):
            href = match.group("href").strip()
            resolved = self._resolve_link(path, href)
            images.append(NoteImage(alt=match.group("alt").strip(), href=href, path=resolved))

        return Note(
            path=path.relative_to(self.root),
            title=title,
            tags=tags,
            body=body.strip(),
            meta=meta,
            images=images,
        )

    @staticmethod
    def _split_frontmatter(raw_text: str) -> tuple[dict[str, Any], str]:
        if not raw_text.startswith("---"):
            return {}, raw_text

        parts = raw_text.split("---", 2)
        if len(parts) < 3:
            return {}, raw_text

        _, frontmatter, body = parts
        meta = yaml.safe_load(frontmatter) or {}
        if not isinstance(meta, dict):
            meta = {}
        return meta, body

    @staticmethod
    def _normalize_tags(value: Any) -> list[str]:
        if isinstance(value, list):
            tags = [str(tag).strip() for tag in value if str(tag).strip()]
        elif isinstance(value, str):
            tags = [part.strip() for part in value.split(",") if part.strip()]
        else:
            tags = []
        return sorted(dict.fromkeys(tags), key=str.casefold)

    @staticmethod
    def _resolve_link(note_path: Path, href: str) -> Path | None:
        if "://" in href:
            return None

        candidate = (note_path.parent / href).resolve()
        return candidate if candidate.exists() else None


class NoteListItem(ListItem):
    def __init__(self, note: Note) -> None:
        self.note = note
        super().__init__(
            Static(note.title, classes="note-item-text", markup=False),
            classes="note-item",
        )


class NotesApp(App[None]):
    CSS_PATH = "textual_notes.tcss"
    TITLE = "Vault Notes"
    SUB_TITLE = "Поиск и просмотр заметок"
    BINDINGS = [
        Binding("/", "focus_search", "Поиск"),
        Binding("ctrl+l", "focus_list", "Список"),
        Binding("ctrl+r", "reload_notes", "Reload"),
        Binding("ctrl+up", "focus_previous_image", "Prev image"),
        Binding("ctrl+down", "focus_next_image", "Next image"),
        Binding("ctrl+o", "open_focused_image", "Open image"),
        Binding("ctrl+q", "quit", "Quit"),
    ]

    def __init__(self, vault_root: Path) -> None:
        super().__init__()
        self.vault_root = vault_root.resolve()
        self.vault = Vault(self.vault_root)
        self.notes: list[Note] = []
        self.filtered_notes: list[Note] = []
        self.selected_note: Note | None = None
        self.focused_image_index: int | None = None

    def compose(self) -> ComposeResult:
        yield Header(show_clock=True)
        yield Input(placeholder="Поиск...", id="search")

        with Horizontal(id="main-layout"):
            with Vertical(id="sidebar"):
                yield ListView(id="note-list")

            with Vertical(id="preview-column"):
                with VerticalScroll(id="preview-scroll"):
                    yield Static(id="preview")

        yield Footer()

    def on_mount(self) -> None:
        self.action_reload_notes()
        self.query_one("#note-list", ListView).focus()

    def action_focus_search(self) -> None:
        self.query_one("#search", Input).focus()

    def action_focus_list(self) -> None:
        self.query_one("#note-list", ListView).focus()

    def action_reload_notes(self) -> None:
        self.notes = self.vault.load_notes()
        self._apply_filter(self.query_one("#search", Input).value if self.is_mounted else "")
        self.notify(f"Загружено заметок: {len(self.notes)}", timeout=2)

    def action_open_focused_image(self) -> None:
        note = self.selected_note
        if note is None:
            self.notify("Сначала выбери заметку", severity="warning")
            return

        if not note.images:
            self.notify("В этой заметке нет картинок", severity="warning")
            return

        index = self.focused_image_index if self.focused_image_index is not None else 0
        index = max(0, min(index, len(note.images) - 1))
        self._open_href(note.images[index].href)

    def action_focus_previous_image(self) -> None:
        self._move_image_focus(-1)

    def action_focus_next_image(self) -> None:
        self._move_image_focus(1)

    def on_input_changed(self, event: Input.Changed) -> None:
        if event.input.id == "search":
            self._apply_filter(event.value)

    def on_list_view_highlighted(self, event: ListView.Highlighted) -> None:
        item = event.item
        if isinstance(item, NoteListItem):
            self._show_note(item.note)

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        item = event.item
        if isinstance(item, NoteListItem):
            self._show_note(item.note)

    def _apply_filter(self, query: str) -> None:
        tokens = [part.lower().lstrip("#") for part in query.split() if part.strip()]
        if not tokens:
            self.filtered_notes = list(self.notes)
        else:
            self.filtered_notes = [
                note
                for note in self.notes
                if all(any(token in tag.casefold() for tag in note.tags) for token in tokens)
            ]

        self._refresh_note_list()

    def _refresh_note_list(self) -> None:
        list_view = self.query_one("#note-list", ListView)
        current_path = self.selected_note.path if self.selected_note else None

        list_view.clear()
        list_view.extend(NoteListItem(note) for note in self.filtered_notes)
        list_view.border_title = f"Заметки {len(self.filtered_notes)}/{len(self.notes)}"

        if not self.filtered_notes:
            self.selected_note = None
            self.focused_image_index = None
            all_tags = sorted(
                {tag for note in self.notes for tag in note.tags},
                key=str.casefold,
            )
            tags_line = "  ".join(f"#{t}" for t in all_tags) if all_tags else "—"
            self.query_one("#preview", Static).update(
                Group(
                    RichMarkdown("# Пусто"),
                    Text(""),
                    Text(f"Доступные теги: {tags_line}"),
                )
            )
            return

        selected_index = 0
        if current_path is not None:
            for index, note in enumerate(self.filtered_notes):
                if note.path == current_path:
                    selected_index = index
                    break

        list_view.index = selected_index
        self._show_note(self.filtered_notes[selected_index])

    def _show_note(self, note: Note) -> None:
        previous_path = self.selected_note.path if self.selected_note else None
        self.selected_note = note
        if not note.images:
            self.focused_image_index = None
        elif previous_path != note.path or self.focused_image_index is None:
            self.focused_image_index = 0
        else:
            self.focused_image_index = min(self.focused_image_index, len(note.images) - 1)
        self.query_one("#preview", Static).update(self._render_note(note))

    def _move_image_focus(self, step: int) -> None:
        note = self.selected_note
        if note is None or not note.images:
            self.notify("В текущей заметке нет картинок", severity="warning")
            return

        current = self.focused_image_index or 0
        self.focused_image_index = (current + step) % len(note.images)
        self._show_note(note)
        self.notify(
            f"Картинка {self.focused_image_index + 1}/{len(note.images)}",
            timeout=1.5,
        )

    def _render_note(self, note: Note) -> Group:
        renderables: list[Any] = []
        body = note.body.strip()
        position = 0
        image_index = 0

        for match in BLOCK_PATTERN.finditer(body):
            start, end = match.span()
            if start > position:
                self._append_markdown_segment(renderables, body[position:start])

            image_href = match.group("image_href")
            if image_href:
                image_alt = match.group("image_alt") or Path(image_href).name
                renderables.append(
                    self._render_image_block(note, image_href, image_alt, image_index)
                )
                image_index += 1
            else:
                formula = (match.group("fence_body") or match.group("display_body") or "").strip()
                if formula:
                    renderables.append(self._render_formula_block(formula))

            position = end

        if position < len(body):
            self._append_markdown_segment(renderables, body[position:])

        if not renderables:
            renderables.append(Text("Пустая заметка"))

        return Group(*renderables)

    def _append_markdown_segment(self, renderables: list[Any], markdown_text: str) -> None:
        segment = markdown_text.strip()
        if not segment:
            return

        segment = INLINE_MATH_PATTERN.sub(self._replace_inline_math, segment)
        renderables.append(RichMarkdown(segment, code_theme="monokai"))

    def _replace_inline_math(self, match: re.Match[str]) -> str:
        source = match.group(1).strip()
        text = LATEX_TO_TEXT.latex_to_text(source).strip()
        return f"`{text or source}`"

    def _render_image_block(self, note: Note, href: str, alt: str, image_index: int) -> Any:
        resolved = self._resolve_note_asset(note, href)

        if resolved is None or not resolved.exists():
            return Text(f"Картинка не найдена: {href}", style="red")

        return RichImage(resolved, width="80%", height="auto")

    def _render_formula_block(self, formula: str) -> Any:
        image_path = self._render_formula_image(formula)
        if image_path is not None:
            return RichImage(image_path, width="auto", height=4)

        fallback = LATEX_TO_TEXT.latex_to_text(formula).strip() or formula
        return Text(fallback)

    def _render_formula_image(self, formula: str) -> Path | None:
        cleaned = " ".join(formula.strip().split())
        if not cleaned:
            return None

        cache_dir = self.vault_root / ".cache" / "formula"
        cache_dir.mkdir(parents=True, exist_ok=True)
        cache_path = cache_dir / f"{hashlib.sha1(cleaned.encode('utf-8')).hexdigest()}.png"

        if cache_path.exists():
            return cache_path

        os.environ.setdefault("MPLCONFIGDIR", "/tmp/mpl")

        try:
            import matplotlib

            matplotlib.use("Agg")
            import matplotlib.pyplot as plt

            figure = plt.figure(figsize=(6, 0.5), dpi=150)
            figure.patch.set_alpha(0.0)
            figure.text(0.02, 0.5, f"${cleaned}$", fontsize=13, color="white", va="center")
            figure.savefig(cache_path, bbox_inches="tight", transparent=True, pad_inches=0.1)
            plt.close(figure)
            return cache_path
        except Exception:
            return None

    def _resolve_note_asset(self, note: Note, href: str) -> Path | None:
        if "://" in href:
            return None
        return ((self.vault_root / note.path).parent / href).resolve()

    def _open_href(self, href: str) -> None:
        note = self.selected_note
        if note is None:
            return

        if "://" in href:
            webbrowser.open(href)
            self.notify(f"Открываю ссылку: {href}", timeout=2)
            return

        resolved = self._resolve_note_asset(note, href)

        if resolved is not None and resolved.exists():
            webbrowser.open(resolved.as_uri())
            self.notify(f"Открываю файл: {resolved.name}", timeout=2)
        else:
            self.notify(f"Файл не найден: {href}", severity="error")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Textual vault viewer")
    parser.add_argument(
        "--vault",
        type=Path,
        default=Path(__file__).parent / "vault",
        help="Путь до vault с подпапкой notes/",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    app = NotesApp(args.vault)
    app.run()


if __name__ == "__main__":
    main()
