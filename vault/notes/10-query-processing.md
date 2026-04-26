---
title: "Обработка запросов в GraphRAG"
tags: ["graphrag", "query-processing", "retrieval", "generation"]
---

# Обработка запросов в GraphRAG

## Типы запросов

| Тип | Описание | Пример |
|-----|----------|--------|
| **Local** | Конкретная информация об сущности | "Кто основал Microsoft?" |
| **Global** | Обобщённый анализ всего датасета | "Основные тренды в AI за 2024 год?" |

## Local Query Pipeline


Эмбеддинги строятся по **обобщениям сущностей** (summary), а не по сырым описаниям.

$$
\mathbf{q} = \text{embed}(\text{Query}) \in \mathbb{R}^{1536}
$$

$$
\{e_1, \dots, e_k\} = \text{TopK}(\text{similarity}(\mathbf{q}, \text{embed}(\text{summary}(E))))
$$

После нахождения релевантных сущностей система расширяет контекст через граф:

| Источник | Содержимое |
|----------|-----------|
| **Чанки текста** | Исходные фрагменты, связанные с найденными сущностями |
| **Сводки сообществ** | Отчёты сообществ, в которых состоят сущности |
| **Связи** | Обобщённые описания рёбер между найденными сущностями |
| **Сводки сущностей** | Consolidated summary каждой найденной сущности |

Все источники ранжируются и обрезаются до размера контекстного окна LLM.

## Global Query Pipeline (Map-Reduce)


Глобальный поиск работает как двухфазный map-reduce:

**Фаза Map** — для каждого сообщества с рейтингом выше порога:
$$
\text{PartialAnswer}_i = \text{LLM}(\text{Query},\ S_i)
$$
Каждый промежуточный ответ содержит ключевые пункты с оценкой важности (0–100).

**Фаза Reduce** — агрегация:
$$
\text{FinalAnswer} = \text{LLM}\!\left(\text{Query},\ \bigcup_i \text{PartialAnswer}_i\right)
$$
LLM синтезирует все промежуточные ответы в один финальный.

> Качество ответа зависит от уровня иерархии сообществ: нижний уровень — детали, верхний — тематические обобщения.

## Алгоритм ранжирования

$$
\text{rank}(e_i) = \alpha \cdot \text{sim}(\mathbf{q}, \mathbf{e}_i)
            + \beta \cdot \text{centrality}(e_i)
            + \gamma \cdot \text{community\_score}(e_i)
$$

где $\alpha = 0.5$, $\beta = 0.3$, $\gamma = 0.2$.

## Формулы центральности

**PageRank**:

$$
PR(v) = \frac{1-d}{N} + d \sum_{u \in \text{in}(v)} \frac{PR(u)}{|\text{out}(u)|}
$$

**Betweenness**:

$$
BC(v) = \sum_{s \neq v \neq t} \frac{\sigma_{st}(v)}{\sigma_{st}}
$$

## Итоговый скор качества ответа

$$
\text{Score} = \sqrt[3]{\text{Relevance} \times \text{Completeness} \times \text{Consistency}}
$$
