# Shaitan

#### Data Processing Framework

---

### Roadmap:

- ✅ Обнаружение gRPC сервисов через reflection
- ◻️ Очереди через центральный сервер
- ◻️ Концепция пайплайнов (эмиттеры и процессоры)
- ◻️ Реализация минимального web-сервера
- ◻️ Нагрузочное тестирование, сравнение с конкурентами

---

### Обнаружение gRPC сервисов через reflection

Надо запустить src/services/python/exe/main.py, а затем worker.
В результате worker запросит информацию о схеме и отправит пример
запроса с помощью JSON.

---

### Очереди через центральный сервер

