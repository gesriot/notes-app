---
title: "Векторные эмбеддинги в GraphRAG"
tags: ["graphrag", "embeddings", "vector-search", "similarity"]
---

# Векторные эмбеддинги в GraphRAG

## Три типа эмбеддингов

GraphRAG использует векторные представления для эффективного поиска:

| Тип | Назначение | Размерность |
|-----|------------|-------------|
| **Text Unit Embeddings** | Векторизация чанков текста | 1536 (OpenAI) |
| **Entity Description Vectors** | Эмбеддинги описаний сущностей | 1536 |
| **Community Summaries** | Векторы резюме сообществ | 1536 |

## Косинусное сходство

$$
\text{similarity}(\mathbf{a}, \mathbf{b}) = \frac{\mathbf{a} \cdot \mathbf{b}}{\|\mathbf{a}\| \|\mathbf{b}\|} = \cos\theta
$$

где $\mathbf{a}, \mathbf{b} \in \mathbb{R}^d$ — векторные представления текстов.

## Модели эмбеддингов

| Модель | Размерность | Особенность |
|--------|------------|-------------|
| text-embedding-3-small | 1536 | Быстрая, экономичная |
| text-embedding-3-large | 3072 | Высокая точность |
| multilingual-e5-large | 1024 | Мультиязычная |

## Евклидово расстояние

$$
d(\mathbf{a}, \mathbf{b}) = \sqrt{\sum_{i=1}^{d} (a_i - b_i)^2}
$$

Чем меньше $d$, тем более похожи объекты.

## Approximate Nearest Neighbor (ANN)

$$
\text{ANN}(\mathbf{q}, k) = \{\mathbf{v}_1, \dots, \mathbf{v}_k\}
$$

где $\mathbf{v}_i$ — топ-$k$ ближайших векторов к запросу $\mathbf{q}$.
