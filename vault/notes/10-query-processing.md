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

$$
\mathbf{q} = \text{embed}(\text{Query}) \in \mathbb{R}^{1536}
$$

$$
\{e_1, \dots, e_k\} = \text{TopK}(\text{similarity}(\mathbf{q}, E))
$$

$$
\text{Context} = \text{GraphTraversal}(\{e_1, \dots, e_k\},\ \text{depth}=2)
$$

## Global Query Pipeline

$$
\forall C_i:\ \text{PartialAnswer}_i = \text{LLM}(\text{Query},\ S_i)
$$

$$
\text{FinalAnswer} = \text{LLM}\!\left(\text{Query},\ \bigcup_i \text{PartialAnswer}_i\right)
$$

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
