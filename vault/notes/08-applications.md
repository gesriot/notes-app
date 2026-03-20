---
title: "Практические применения GraphRAG"
tags: ["graphrag", "applications", "use-cases", "industry"]
---

# Практические применения GraphRAG

## Основные сферы применения

| Домен | Граф | Применение |
|-------|------|------------|
| E-commerce | Товары + Пользователи + Покупки | Рекомендации товаров |
| Streaming | Контент + Пользователи + Просмотры | Рекомендации фильмов |
| Финансы | Транзакции + Аккаунты + Паттерны | Обнаружение мошенничества |
| B2B | Клиенты + Контракты + Продукты | Customer 360 |

## Query-Focused Summarization (QFS)

Суммаризация больших документов с фокусом на запрос пользователя.

$$
\text{Quality} = \alpha \cdot \text{Comprehensiveness} + \beta \cdot \text{Diversity}
$$

где $\alpha = 0.6$, $\beta = 0.4$.

## Обнаружение мошенничества

Граф транзакций помогает выявлять аномальные паттерны:

$$
\text{Anomaly}(v) = \begin{cases}
1 & \text{если } \text{score}(v) > \theta \\
0 & \text{иначе}
\end{cases}
$$

## Customer 360

Полное представление о клиенте через граф:

$$
G_\text{customer} = \bigcup_{i=1}^{n} (\text{Entity}_i,\ \text{Relation}_i,\ \text{Attribute}_i)
$$

## Формула полезности решения

$$
U(d) = \sum_{i=1}^{n} w_i \cdot \text{impact}_i(d,\ \text{KG})
$$

где $w_i$ — вес фактора, $\text{impact}_i$ — влияние на основе графа знаний.
