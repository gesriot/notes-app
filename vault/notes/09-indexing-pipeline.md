---
title: "Индексация данных в GraphRAG"
tags: ["graphrag", "indexing", "pipeline", "data-processing"]
---

# Индексация данных в GraphRAG

## Pipeline индексации

### Этап 1: Разбиение текста на чанки

$$
\text{Document} \xrightarrow{\text{chunk}} \{C_1, C_2, \dots, C_n\}
$$

Чанки перекрываются для сохранения контекста:

$$
C_i \cap C_{i+1} \neq \emptyset,\quad |C_i \cap C_{i+1}| \approx 200\ \text{токенов}
$$

### Этап 2: Извлечение триплетов

| Операция | Вход | Выход |
|----------|------|-------|
| Entity Recognition | Текстовый чанк $C_i$ | Множество $E_i$ |
| Relation Extraction | Пара $(e_1, e_2) \in E_i^2$ | Отношение $r$ |
| Claim Extraction | Чанк $C_i$ | Утверждения $A_i$ |

### Этап 3: Построение графа

$$
V = \bigcup_{i=1}^{n} E_i, \qquad
T = \{(e_s, r, e_t) : e_s, e_t \in V,\ r \in E\}
$$

### Этап 4: Community Detection

$$
\text{Leiden}(G) \to \{C_1, C_2, \dots, C_k\}
$$

### Этап 5: Генерация сводок

$$
S_j = \text{LLM}\!\left(\text{summarize}\!\left(\bigcup_{e \in C_j} \text{desc}(e)\right)\right)
$$

## Формула сложности

$$
\text{Cost} = O(n \cdot t_\text{LLM}) + O(|V|^2 \cdot t_\text{Leiden}) + O(k \cdot t_\text{summary})
$$

где $n$ — число чанков, $|V|$ — число узлов, $k$ — число сообществ.
