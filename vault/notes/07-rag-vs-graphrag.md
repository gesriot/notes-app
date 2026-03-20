---
title: "Сравнение RAG и GraphRAG"
tags: ["graphrag", "rag", "comparison", "analysis"]
---

# Сравнение RAG и GraphRAG

## Архитектурные различия

| Характеристика | Traditional RAG | GraphRAG |
|----------------|-----------------|----------|
| **Хранилище** | Векторная БД | Векторная БД + Граф |
| **Индексация** | Простые чанки | Сущности + Связи + Сообщества |
| **Поиск** | Семантическое сходство | Граф + Семантика |
| **Контекст** | Изолированные фрагменты | Связанные сущности |
| **Точность** | Базовая | +72–83% |

## Формулы обоих подходов

### Traditional RAG

$$
\text{Answer} = \text{LLM}(\text{Query},\ \text{TopK}(\text{VectorSearch}(\text{Query})))
$$

### GraphRAG

$$
\text{Context} = \text{Summaries}(\text{RelatedCommunities}(\text{GraphTraversal}(\text{Query})))
$$

$$
\text{Answer} = \text{LLM}(\text{Query},\ \text{Context})
$$

## Производительность (Microsoft Research, 2024)

| Метрика | RAG | GraphRAG | Улучшение |
|---------|-----|----------|-----------|
| Comprehensiveness | 58% | 83% | +43% |
| Diversity | 52% | 82% | +58% |
| Token Usage | 100k | 3k | -97% |

## Формула эффективности

$$
\text{Efficiency} = \frac{\text{Quality}}{\text{Tokens Used}}
$$

$$
\frac{E_\text{GraphRAG}}{E_\text{RAG}} = \frac{0.83 / 3000}{0.58 / 100000} \approx 47.7
$$

GraphRAG **в 48 раз эффективнее** при query-focused summarization.
