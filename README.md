# Shaitan

#### Data Processing Framework

---

### Roadmap:

- ✅ Обнаружение gRPC сервисов через reflection
- ✅ Очереди через центральный сервер
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

Запустить main.py, server и worker. Worker запросит инфу о схеме,
зарегается на сервере и на сервер можно будет отправить джобу
в виде JSON. В логах будет результат отработки.

### Концепция пайплайнов

#### Определения

**Эмиттер** - штука, которая стримит данные и инициирует пайплайн.

**Процессор** - принимает на вход данные и что-то с ними делает.

**Пайплайн** - поток данных, может быть вложенным, имеет id.

**Джоба** - штука, у которой есть вход и выход.

**Схема пайплайна** - объединение джоб, может быть тоже джобой.


#### Как это работает

Схему пайплайна будем описывать в виде toml:

```yaml

pipelines:
  - preprocessing:
      jobs:
        - processors:
            input: polygon.objects.File
            output: polygon.objects.FileReport
            executors:
              - polygon.packages.Archive.Process
              - polygon.packages.Exe.Process
              - polygon.packages.Doc.Process
            accepts:
              - watcher
        - limiter:
            input: polygon.objects.FileReport
            output: bool
            register: has_reached_limit
            executors:
              - polygon.core.PackageLimiter
        - watcher:
            input: polygon.objects.FileReport
            output: polygon.objects.File
            executors:
              - polygon.core.DirWatcher
            accepts:
              - process_package
            when:
              - not has_reached_limit
        - combiner:
            input: streaming polygon.objects.FileReport
            output: polygon.objects.FileReports
            executors:
              - 
            accepts:
              - process_package
            when:
              - all_finished:
                  - processors
                  - limiter
                  - watcher
        - filters:
            input: polygon.objects.FileReports
            output: polygon.objects.FileReports
            accepts:
              - combiner
            executors:
              - polygon.filters.SafePdf.Filter

```

For package:

```yml

pipelines:
  - process_package:
      executors:
        - Exe
        - Pdf
        - Doc
      jobs:
        - check_if_suitable:
            accepts: input
            register: is_suitable
        - check_if_encrypted:
            accepts: input
            register: is_encrypted
            when: is_suitable
        - generate_passwords:
            register: password_variant[]
            when: is_encrypted
        - check_password:
            accepts: (input, password_variant)
            register: password
            until: not password.valid
            count: num_password_checks
        - decrypt:
            accepts: (input, password)
            register: decrypted
            when: password.valid
        - get_summary:
            accepts:
              - decrypted
              - input
            register: output
            when:
              - is_suitable and not is_encrypted
              - is_suitable and decrypted
              - is_suitable and password_variant[].len == num_password_checks
  - process_packages:
      jobs:
        - watch_task:
            accepts: input
            register: file[]
        - watch_dir:
            accepts: path
            register: file[]
            until: not limit_reached
        - process_package:
            accepts: file
            register: file_report[]
            when: not limit_reached
        - limiter:
            accepts: file_report
            register: limit_reached
        - next_hop:
            accepts: file_report
            register: path
            when: not limit_reached
        - flatten_reports:
            accepts: file_report
            register: output
  - filter_packages:
      executors:
        - SafePdf
        - Whitelist
      jobs:
        - get_filtered_packages:
            accepts: input
            register: filtered_package[]
        - flatten_filtered:
            accepts: filtered_package
            register: output
  - preprocessing:
      jobs:
        - process_packages:
            accepts: input
            register: file_reports
        - filter_packages:
            accepts: file_reports
            register: filter_reports
        - reporter:
            accepts: (file_reports, filter_reports)
            register: output

```

accepts - параметры, с которыми мы вызываем эту джобу.
input и все registered - это переменные, храним, пока есть пайплайн, потом дропаем.
when - управляет потоком, что-то может ожидать и так далее.

register: is_suitable - меняет состояние на defined и можно запустить зависимые джобы, например,
check_if_encrypted. Аналогично с is_encrypted.

однако, register: password_variant работает другим образом, он генерит поток данных
по сути, список паролей, которые затем проверяются в check_password и тем самым продолжают поток (password).

джоба decrypt срабатывает только когда в потоке password будет password.valid,

вероятно, еще нужно:
- until: not password.valid - типа у generate_passwords или check_password, очевидно
- finish_flag: not_decrypted - когда поток закончился и не смогли подобрать пароль.

---

Пайплайн (pipeline) - набор работ (job) и исполнителей (executors), порядок 
выполнения которых определяется, по сути, нотификаторами: 

* on_variable_defined - вызывается, когда переменная определена;
* on_variable_changed - вызывается, когда переменная изменена;
* on_pipeline_finished -

Работа (job) - gRPC метод или пайплайн, для которого определен один или несколько
нотификаторов, например: 

```yml
watch_task:
  accepts: input
```

определяет нотификтор `on_variable_defined`, специальное имя `input` - означает точку
входа для данных пайплайна, таким образом, такая работа будет выполнена один раз
в момент инициализации пайплайна (т.е. когда input будет определен).

Ключевое слово `accepts` может принимать следующие значения:
* `var1` - имя переменной, которую ожидает работа
* `var1, var2` - список переменных, работа будет запущена при определении одной из них;
* `(var1, var2)` - ...?

Для того чтобы направить данные от одной работы к другой, необходимо в поле `accepts`
указать идентификатор результата работы, например, `watch_task.result`:

```yml
process_package:
  accepts:
    - watch_task.result
```

Если результатом работы является поток, то элементы потока асинхронно подаются на вход
зависимой работы, т.е. для примера выше, например, пайплайн следующий:

```yml
watch_task:
  accepts: input
process_package:
  accepts: watch_task.result
```

Пусть результатом работы `watch_task` будет поток файлов `File`, тогда входом
работы `process_package` будет просто файл `File`. Пусть при этом `process_package` 
так же порождает поток `FileReport` и есть задача объединить все такие отчеты, т.е.:

```text
input --- file1 --- file_report_1
      \         \
       \         file_report_2
        \
          file2 --- file_report_3
                \ 
                 file_report_4
```

В этом случае необходимо выделить этот набор работ в отдельный пайплайн, выходом
которого будет являться поток `FileReport`, для этого необходимо определить `output` работу:

```yml
output:
  accepts: process_package.result
  streaming: yes
```

В данном случае дополнительно указано ключевое слово `streaming`, которое указывает
на тот факт, что выходные значения будут генерироваться многократно, без дополнительных
условий признаком окончания потока будет являться завершение всех работ и отсутствие 
очередей. 

Для того чтобы ограничить поток выполнения, для этого существует ключевое слово `limit`,
которое применимо только для работ с `streaming: yes`.

```yml
output:
  accepts: process_package.result
  streaming: yes
  limit: 10
```

В результате полученный пайплайн на некоторых вход `input` генерирует поток `FileReport` 
с числом элементов не больше 10 и выглядит следующим образом:

```yml
generate_file_reports:
  jobs:
    watch_task:
      accepts: input
    process_package:
      accepts: watch_task.result
    output:
      accepts: process_package.result
      streaming: yes
      limit: 10
```

Теперь пример с перебором паролей: на вход подается файл, необходимо подобрать к нему пароль.

```yml
bruteforce_password:
  jobs:
    generate_passwords:
      accepts: input
    join_file_password:
      join_map:
        output_type: CheckPasswordParameters
        map:
          path: {{ input.path }}
          password: {{ generate_passwords.result }}
    check_password:
      accepts: join_file_password.result
    output:
      accepts: check_password.result
      when: check_password.result.valid
      else:
        error: password not found
```

Выше определяется пайплайн, который делает следующее: 

* `generate_passwords` генерирует поток `string` - возможных паролей;
* `join_file_password` объединяет `input` и `generate_password` и заполняет структуру,
  указанного типа `CheckPasswordParameters`;
* `check_password` просто проверяет пароль у файла;
* `output` выдает результат, если пароль найден и ошибку, если не найден.

Новые возможности применены здесь:

* `join_map` - особый тип работы, который выполняется внутри движка, джойнит типы и
и порождает новую сущность указанного типа с нужным заполнением;
* `when` - условие, при котором работа вызывается;
* `else.error`/`else.default` - значение либо ошибка в случае, если пайплайн закончился, 
  а значение так и не вернулось 

Визуализация 1:

```text
input _______ password1 ___ input + password1 ___ false
      \_____ password2 ___ input + password2 ___ true ___ output!
       \___ password3 ___ input + password3 ___ X (not called)
        \_ password4 ___ X (not called)
       
```

Визуализация 2:

```text
input _______ password1 ___ input + password1 ___ false
      \_____ password2 ___ input + password2 ___ false
       \___ password3 ___ input + password3 ___ false
        \_ password4 ___ input + password4 ___ false
        
        ___ password not found ___ output!
```

