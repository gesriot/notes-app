# Textual Notes Prototype

TUI-прототип хранилища заметок с:

- поиском по тегам;
- списком заметок слева;
- preview справа;
- отображением картинок прямо в терминале;
- улучшенным рендером формул: inline -> Unicode, block -> терминальный formula block.

## Запуск

```bash
python3 -m venv .venv
./.venv/bin/pip install -r requirements.txt
./.venv/bin/python textual_notes.py
```

Если хочешь открыть другой vault:

```bash
./.venv/bin/python textual_notes.py --vault /path/to/vault
```

## Ограничения

- сложный LaTeX вроде некоторых окружений `\begin{...}` может падать в Unicode fallback вместо “красивой” формулы;
- картинки в TUI не будут выглядеть как в браузере, но уже отображаются прямо в preview;
- `Ctrl+Up` / `Ctrl+Down` переключают текущую картинку внутри заметки, `Ctrl+O` открывает именно ее;
- фильтр ищет только по тегам.
