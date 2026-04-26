---
title: "Построение графов знаний из текста с помощью LLM"
tags: ["graphrag", "knowledge-graph", "llm", "structured-extraction", "pydantic", "neo4j"]
---

# Построение графов знаний из текста с помощью LLM

## Проблема: текстовые данные без структуры

Большая часть корпоративной информации хранится как неструктурированный текст (договоры, документы, отчёты). При чистом векторном поиске возникают проблемы:

- Чанки из разных документов смешиваются → неправильный контракт в ответе
- Невозможно считать, фильтровать, агрегировать
- Нельзя ответить: "Сколько активных контрактов с ACME Inc.?"

**Решение**: извлечь структуру с помощью LLM и импортировать в граф.

## Шаг 1: Определение схемы извлечения (Pydantic)

LLM с функцией **Structured Outputs** (OpenAI) гарантирует, что ответ соответствует заданной схеме.

```python
from pydantic import BaseModel, Field
from typing import List, Optional

class Organization(BaseModel):
    """Организация, участвующая в контракте."""
    name: str
    role: str = Field(..., description="Роль: Client или Provider")

class Contract(BaseModel):
    """Ключевые детали контракта."""
    contract_type: str = Field(..., enum=["Service Agreement", "NDA", "Licensing Agreement"])
    parties: List[Organization]
    effective_date: str = Field(..., description="Дата в формате yyyy-MM-dd")
    term: str = Field(..., description="Срок действия и условия продления")
    total_amount: Optional[float]
    end_date: Optional[str]
```

**Важные приёмы:**
- `description` в `Field` — мини-инструкция для LLM, как извлекать данное поле
- `enum` — ограничивает допустимые значения
- `Optional` — поле может отсутствовать в документе
- Имя класса + docstring дают LLM общее понимание цели

## Шаг 2: Вызов LLM для извлечения

```python
def extract(document: str, model="gpt-4o-2024-08-06", temperature=0):
    response = client.beta.chat.completions.parse(
        model=model,
        temperature=temperature,
        messages=[
            {"role": "system", "content": system_message},
            {"role": "user", "content": document},
        ],
        response_format=Contract,
    )
    return json.loads(response.choices[0].message.content)
```

`temperature=0` — детерминированный вывод для структурированного извлечения.

## Шаг 3: Импорт в Neo4j

Модель графа контракта:
```
Contract ──HAS_PARTY──► Organization ──LOCATED_AT──► Location
    │
    └──HAS_CHUNK──► Chunk (исходный текст + embedding)
```

```python
neo4j_driver.execute_query("""
MERGE (contract:Contract {id: randomUUID()})
SET contract += {contract_type: $data.contract_type, ...}
WITH contract
UNWIND $data.parties AS party
MERGE (org:Organization {name: party.name})
MERGE (org)-[:HAS_PARTY]->(contract)
""", data=extracted_data)
```

## Шаг 4: Разрешение сущностей

После импорта нескольких документов возникают дубликаты:

```
UTI Asset Management Company
UTI Asset Management Company Limited  ← одна и та же организация
UTI Asset Management Company Ltd
```

**Entity Resolution** — объединение вариантов в один узел. Стратегии:
- Точное совпадение строк (нормализация)
- Нечёткое совпадение (Levenshtein distance)
- LLM для семантического сравнения

Нет универсального решения — каждый домен требует своего подхода.

## Шаг 5: Добавление неструктурированных данных

После структурированных данных добавляем текстовые чанки для семантического поиска:

```python
# Разбиваем исходный договор на чанки
chunks = chunk_text(contract_text, chunk_size=500, overlap=40)
embeddings = embed(chunks)

# Связываем чанки с узлом контракта
neo4j_driver.execute_query("""
UNWIND $chunks AS chunk
MERGE (c:Chunk {id: chunk.id})
SET c.text = chunk.text, c.embedding = chunk.embedding
MERGE (contract:Contract {id: $contract_id})-[:HAS_CHUNK]->(c)
""", chunks=..., contract_id=...)
```

## Итог: гибридные запросы

Теперь можно отвечать на оба типа вопросов:

| Вопрос | Метод |
|--------|-------|
| "Сколько активных контрактов с ACME?" | Cypher по структурированным данным |
| "Найди условия оплаты в контракте ACME" | Векторный поиск по чанкам + фильтр по Contract |
| "Перечисли все стороны контракта №123" | Cypher через HAS_PARTY |
| "Найди похожие условия расторжения" | Векторный поиск по чанкам |

## Когда структурирование необходимо

Структурировать данные перед RAG стоит тогда, когда:
1. Нужна точная атрибуция (данные из конкретного документа)
2. Требуется агрегация (подсчёт, суммирование, группировка)
3. Разные документы описывают одну предметную область (контракты, отчёты, профили)
4. Важна целостность данных (нельзя путать клиентов)
