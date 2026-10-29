# Диагностика редактора

Наблюдайте запуск и рабочее пространство редактора в именованных
debug-программах. Для действий через UI служит [автоматизация приложения](application.md).
Это не API плагинов редактора: инспекторы проекта, окна, Save и Play здесь не доступны.
Команды подготовки стендов описаны в отдельном [тестовом справочнике](testing.md).

Декларации:
[editor.d.luau](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/types/editor.d.luau)
(English).
Установка области:
[scripting.rs](https://github.com/kedra-engine/kedra/blob/main/crates/kedra_editor/src/scripting.rs)
(English).
Привязки: `crates/kedra_editor/src/scripting.rs`; сериализованные наблюдения:
`crates/kedra_editor/src/remote.rs` и `src/workspace/`.
Редактор устанавливает эту область для `editor.startup.forms` в отладочной диагностике,
а не для скриптов проекта.

## Функции наблюдения

```luau
ctx.editor.state: () -> EditorState
ctx.editor.workspace: () -> Workspace
```

`EditorContext` объединяет `AppContext` и `{editor: EditorApi}`.
Обе функции синхронно возвращают независимые JSON-совместимые снимки через нативные методы.
Им нужен живой Scope; они отзываются вместе с программой.
Неизменяемая таблица editor не содержит `project`, `save`, `play`, выполнения исходников
или произвольных команд. `editor.project` — отдельный нативный метод удалённого доступа,
не `ctx.editor`. Для действий через UI используйте установленный `ctx.app`;
наблюдение не меняет состояние.

## Состояние startup

Поля `EditorState`:

| Поле | Контракт |
| --- | --- |
| `phase` | `"boot" \| "projects" \| "loading" \| "editor"` |
| `generation` | Поколение менеджера проектов, не дескриптор скрипта |
| `preferences` | Нативный путь истории/настроек или null; только наблюдение |
| `busy`, `ready` | Ожидающая задача проекта / готовность модели |
| `error`, `notice` | Ошибка загрузки или null и текущая строка уведомления |
| `project` | `Project` или null: `name`, нативный `root`, `format` |
| `entry` | `Entry` или null: `mode`, `name`, `directory`, `error` |

`Entry.mode` — `"create" | "open"`; строки формы — черновики и сообщения
проверки, а не сохранённые правки проекта. Поля с null используют `json.null`; сужайте тип.
Нативные пути файловой системы здесь — данные протокола, а не подписи для UI.
UI редактора должен показывать относительные пути ресурсов проекта, не абсолютные пути.

`src/scripting/startup.luau` использует эти функции для наблюдения форм менеджера проектов
и нативного ввода. Сокращённый фрагмент предполагает изолированный запуск без открытой формы:

```luau
function run(ctx: EditorContext, args)
    wait.until_true("project manager", function()
        return ctx.editor.state().phase == "projects"
    end, 40000)
    assert(ctx.editor.state().preferences == args.preferences, "Refusing personal editor preferences")
    assert(ctx.editor.workspace().bound == false, "Startup invented a bound workspace")
    ctx.app.click("projects.new")
    wait.until_true("create form", function()
        return ctx.editor.state().entry ~= json.null
    end, 40000)
    ctx.app.field("entry.name", "QA Кедра")
    ctx.app.click("entry.cancel")
end
```

Это адаптация, не произвольный удалённо загружаемый скрипт. Полная программа также проверяет
фокус модальных окон, валидацию, геометрию и захваты; внешние тесты на Rust обеспечивают
изолированные настройки, подготовку и оценку результатов.

## Структура workspace

`Workspace` — `{bound: false}` до привязки рабочего пространства, иначе:

| Поле | Контракт |
| --- | --- |
| `bound`, `native_alive` | Привязка существует; корневая сущность и нативное окно живы |
| `main_host`, `revision` | Идентификатор логического хоста панелей и ревизия раскладки |
| `layout` | Снимок `DockLayout`, не изменяемый объект команд |
| `documents` | Наблюдения сессий документов без изменения данных |
| `scene_views` | Соответствие идентификатора представления и `SceneView` |
| `navigation_input` | Диагностика нативных жестов, измерений и ввода |

`DockLayout` содержит `version`, `hosts`, `nodes`, `views`, `hidden`:

- Хосты содержат идентификаторы узлов `root` и `maximized` или null.
- Узлы имеют форму `{Group={tabs: {string}, active: string}}` или
  `{Split={axis: "Horizontal" | "Vertical", fraction: number, first: string, second: string}}`.
- Представления содержат `kind` и `restore_key`, допускающий null.
- Скрытые представления содержат `host`, `group`, `index` (нативный индекс, не индекс массива Lua).

`Documents` содержит `active` и `pending`, допускающие null, логический `read_only` и `documents`.
Запись документа содержит `document`, `path`, `view` и `selection`, допускающий null.
Выделение `NodeAddress` содержит `instances: {string}` и `node: string`; сохранённые
идентификаторы не становятся дескрипторами живых сущностей. Это не Save документов.

`SceneView` — `{ready: false}` либо снимок готового представления с `spatial`,
допускающими null полями `translation`, `rotation`, `pivot`, `speed` и логическими полями
`orthographic`, `overlays`, `editor_lighting`, `aligning`, а также `ready`.
Готовность и геометрия не подтверждают качество нативного кадра или принятие ввода.

`NavigationInput` содержит `gesture`, `pending_host`, `menu_nodes`, `measuring`, `preview_ready`,
`rendered`, допускающий null, `revision`, `capture={view,window}` или null и `viewports`.
У каждой области просмотра есть `view`, прямоугольник `rect` в логических пикселях или null,
`eligible_windows`, `pointer_windows`.
Оба массива окон содержат десятичные строки `WindowId`, не числа Luau.
`crates/kedra_editor/src/workspace/scene_input.rs::diagnostics` предоставляет эти строки;
`crates/kedra_editor/types/editor.d.luau` объявляет оба массива как `{WindowId}`.
Не разбирайте эти строки: это наблюдения, не дескрипторы сущностей. Координаты прямоугольника,
ревизии и счётчики остаются числовыми.

## Граница проверки

Запуск, стыковка панелей, ввод и отрисовка требуют нативных тестов; одних снимков состояния
недостаточно. [Руководство редактора](../../manual/editor/index.md) описывает инструменты
без изменения исходников;
[программы автоматизации](../../manual/automation/programs-and-diagnostics.md) — вызов сценариев.
