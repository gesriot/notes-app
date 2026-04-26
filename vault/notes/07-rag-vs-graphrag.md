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

> **Важная оговорка**: эта цифра относится к *Query-Focused Summarization* (глобальные запросы). При локальном поиске GraphRAG тоже потребляет меньше токенов за счёт предварительных сводок. Но **глобальный поиск** GraphRAG обрабатывает все сводки сообществ → потребление токенов может быть выше, чем у простого RAG на конкретный вопрос.

## Когда использовать каждый подход

| Сценарий | Рекомендация |
|----------|-------------|
| Конкретный факт ("Кто основал Microsoft?") | Traditional RAG или Local GraphRAG |
| Тематический анализ всего корпуса | Global GraphRAG |
| Структурированные данные (фильтрация, агрегация) | text2cypher |
| Несколько типов данных и вопросов | Agentic RAG |
| Простой FAQ-чатбот | Traditional RAG достаточно |
| Юридические/медицинские документы с атрибуцией | GraphRAG с построением KG |

## Эволюция подходов в порядке сложности

```
Базовый RAG
    → Гибридный RAG (вектор + BM25)
        → Advanced RAG (step-back + parent document)
            → GraphRAG (Local)
                → GraphRAG (Global / QFS)
                    → Agentic RAG (несколько ретриверов)
```

Каждый уровень даёт прирост качества, но требует больше инфраструктуры и стоимости индексации.

## Стоимость индексации GraphRAG

При построении индекса GraphRAG LLM вызывается многократно:

$$
\text{Cost}_\text{index} = \underbrace{O(n \cdot t_\text{LLM})}_{\text{извлечение}} + \underbrace{O(|V| \cdot t_\text{LLM})}_{\text{обобщение сущностей}} + \underbrace{O(k \cdot t_\text{LLM})}_{\text{обобщение сообществ}}
$$

Это разовая стоимость при построении индекса. При обновлении данных нужно переиндексировать — существенный недостаток при динамичных данных.
