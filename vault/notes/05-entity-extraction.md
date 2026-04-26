---
title: "Извлечение сущностей и связей"
tags: ["graphrag", "entity-extraction", "llm", "nlp"]
---

# Извлечение сущностей и связей

## Процесс извлечения

GraphRAG использует LLM для автоматического извлечения сущностей из текста.

![Entity Extraction](../images/entity-extraction.jpg)

## Типы сущностей

| Тип | Примеры | Обозначение |
|-----|---------|-------------|
| **PERSON** | Albert Einstein, Steve Jobs | $E_\text{person}$ |
| **ORGANIZATION** | Microsoft, Apple, IBM | $E_\text{org}$ |
| **LOCATION** | Paris, USA, Silicon Valley | $E_\text{loc}$ |
| **CONCEPT** | GraphRAG, AI, Machine Learning | $E_\text{concept}$ |
| **EVENT** | Merger, Conference, Release | $E_\text{event}$ |

## Извлечение связей

Связь представляется как:

$$
r: E \times E \to R
$$

**Примеры**:
- `(Microsoft, developed, GraphRAG)`
- `(GraphRAG, improves, RAG)`
- `(RAG, uses, LLM)`

## Формат вывода при извлечении (промпт MS GraphRAG)

Каждая **сущность** извлекается в формате:
```
("entity" | entity_name | entity_type | entity_description)
```

Каждая **связь**:
```
("relationship" | source_entity | target_entity | relationship_description | relationship_strength)
```

`relationship_strength` — числовая оценка силы связи.

## Шаг обобщения сущностей

После извлечения одна сущность может иметь несколько описаний из разных чанков:

```
Орест — сын Агамемнона, убивший Эгисфа.
Орест — человек, который должен был отомстить Эгисфу.
Орест прославился тем, что отомстил за убийство своего отца.
```

LLM объединяет их в единое связное резюме:

$$
\text{summary}(e) = \text{LLM}\!\left(\text{merge}(\text{desc}_1, \text{desc}_2, \dots, \text{desc}_n)\right)
$$

Аналогично для связей: несколько описаний одной пары сущностей → одно резюме.

## Проблема суперузлов

Суперузел — сущность, встречающаяся во множестве чанков и накапливающая огромное количество описаний. При большом корпусе требует отдельной стратегии обработки.

## Доменные типы сущностей (пример: «Одиссея»)

В книге для текста «Одиссеи» использовались типы:
`PERSON`, `ORGANIZATION`, `LOCATION`, `GOD`, `EVENT`, `CREATURE`, `WEAPON_OR_TOOL`

Типы сущностей подбираются под конкретный домен — их нужно определять на основе датасета и целевых вопросов.

## Метрики точности

$$
\text{Precision} = \frac{TP}{TP + FP}, \qquad \text{Recall} = \frac{TP}{TP + FN}
$$

При использовании GPT-4: Precision ≈ 0.92, Recall ≈ 0.87.
