# Программы и модули Luau

Точные вызовы описаны в [справочнике runtime](../../api/luau/runtime.md),
[привязках Bevy](../../api/luau/bevy.md) и
[контексте переключателя игрового хоста](../../api/luau/game-host.md).

**Behavior** подходит для постоянного состояния экземпляра и обратных вызовов без
приостановки. **Sequence** — для одной конечной операции, которая может ждать и возвращать
собственные данные. **Module** позволяет разделять обычный код Luau. Импорт модуля
не запускает другую программу.

Граница разрешения выполнения описана в разделах
[ответственности приложения-хоста на Rust](index.md) и
[загрузки и гидратации сцен](../scenes/lifecycle.md);
открытие документа в редакторе не выполняет эти программы.

Каждый разрешённый запуск имеет независимую VM. Behavior владеет изменяемым `self`;
контекст полномочий `ctx` доступен только для чтения. Объявляйте функции жизненного цикла
на верхнем уровне, а не полями возвращаемой таблицы. Behavior требует хотя бы одну из
`start`, `update`, `fixed_update`, `stop`. Их сигнатуры: `start(self, ctx)`,
`update(self, ctx, dt)`, `fixed_update(self, ctx, dt)` и `stop(self, ctx, reason)`.

`start` вызывается один раз непосредственно перед первым update- или fixed-вызовом,
в том числе при динамическом прикреплении Behavior; это не однократный `Startup` Bevy.
Обратные вызовы не могут приостанавливаться. Sequence объявляет `run(ctx, args)`;
у неё нет состояния и хуков Behavior. Неиспользуемые параметры можно опустить,
но приложение-хост всё равно передаёт документированный набор аргументов.

## Настоящий переключатель: Rust разрешает, Luau реагирует

При монтировании каноническая тестовая игра находит спроецированный элемент `game.switch`
и разрешает выполнение следующим **фрагментом** из `src/scripting.rs`. Здесь `world` и
`target` уже принадлежат смонтированному нативному UI. Это не отдельная функция настройки.

```rust
let mut program = Program::behavior(
    "game.switch",
    "scripts/Native/switch_behavior.luau",
    include_str!("../scripts/Native/switch_behavior.luau"),
    |_, _, _| Ok(()),
);
program.params = json!({"color":[30,100,190,255]});
program.validate_params = validate;
program.events = &["activate"];
program.sequences.push(Program::sequence(
    "game.switch.activate",
    "scripts/Native/switch_activate.luau",
    include_str!("../scripts/Native/switch_activate.luau"),
    |lua: &lua::Lua, scope, ctx| kedra_script_bevy::install_ui(lua, scope, ctx),
));
kedra_script::attach(world, target, program, Value::Null)?;
```

Этот исходник импортирует `kedra_script::{Program, ScriptError, lua}` и
`serde_json::{Value, json}`. Его функция `validate` принимает ровно одно поле `color`
из четырёх целых беззнаковых байтов в `0..=255`; остальные значения отклоняются до выделения
ресурсов. Приложение-хост устанавливает `LuauPlugin` через `KedraAppPlugin`, а также
`ScriptBevyPlugin` для передачи нативного `Activate`. Пустые привязки родителя намеренно
не дают ему API изменения UI.

Это полный исходник Behavior игры, кроме директивы линтера:

```lua
-- The native switch host authorizes this Behavior only through attach.
type Context = BehaviorContext & {entity: EntityHandle, params: {color: {number}}}
type State = {activation: SequenceHandle?}

function start(self: State, ctx: Context)
    ctx.events.on(ctx.entity, "activate", function(_payload)
        if self.activation then
            ctx.sequences.cancel(self.activation)
        end
        self.activation = ctx.sequences.start("./switch_activate", {color = ctx.params.color})
    end)
end
```

Отдельно допущенный исходник дочерней программы:

```lua
function run(ctx: UiContext, args: {color: {number}})
    ctx.ui.set_background(ctx.entity, args.color)
    ctx.log.info("Switch activated")
end
```

Behavior подписывается на свой прикреплённый объект и запускает только разрешённую
дочернюю программу. Она получает ту же цель и владельца жизненного цикла, но свою VM
и только UI-привязки. События и полномочия родителя не наследуются. Повторная активация
отменяет предыдущую дочернюю программу; уже применённое изменение цвета не откатывается.
Безоконный контрактный тест проверяет это взаимодействие программ, но не физические
щелчки и отрисованные кадры.

## Ожидание внутри Sequence

Следующий пример runtime использует определённую приложением-хостом привязку
`ctx.game.score()`; это не встроенная игровая служба. При разрешении исходника
приложение-хост должно предоставить эту привязку.

```lua
function run(ctx, args)
    wait.updates(3)
    wait.fixed(2)
    wait.game_seconds(0.25)
    wait.real_seconds(0.01)
    wait.until_true("positive score", function() return ctx.game.score() > 0 end, 5000)
    return {name = "Кедра", ready = true}
end
```

Ожидания используют выполнения расписаний и часы Bevy, а не число отрисованных кадров.
Sequence возобновляются в `Last`; ожидание фиксированных шагов не переносит дальнейший код
в `FixedUpdate`. Изучите
[часы и ограничения](lifetime-and-capabilities.md#часы-и-бюджеты-выполнения)
перед использованием ожидания как условия готовности или таймера, зависящего от паузы.

## Допускайте модули явно, не ищите их на диске

`Program::sources` содержит явно допущенный снимок `Sources`. Путь входного файла
канонический, относительно проекта, например `scripts/entry.luau`; это не полный путь
ассета сцены. Обычные импорты: `require("./lib/counter")`,
`require("../shared/module")` и `require("@game/lib/counter")`.

Для примера с псевдонимом включите ту же `.luaurc` в runtime-снимок:

```json
{"aliases":{"game":"./scripts"}}
```

**Фрагмент Rust** из контрактного теста модулей добавляет файлы в существующий `program`:

```rust
program.sources.insert(".luaurc", r#"{"aliases":{"game":"./scripts"}}"#)?;
program.sources.insert("scripts/lib/counter.luau", "return {value = 1}")?;
```

Относительный импорт счётчика и импорт через псевдоним используют общий кэш внутри одной VM;
это проверяет контрактный тест модулей. Второй запуск получает новое состояние модулей.
Конструкторы удобны, если одной VM
нужны разные объекты; передавайте `ctx` явно операциям, которым нужны полномочия.
Поддерживаются модули `init` в каталогах; импорты не содержат `.luau`/`.lua`.
Неоднозначные расширения, отсутствующие модули, неканонические/повторные пути
и псевдонимы внешней файловой системы приводят к ошибке.

Каждый файл ограничен 64 KiB. Снимок вместе с входным файлом — не более 256 файлов / 8 MiB.
Неявного поиска на диске и чтения внешней конфигурации нет. Псевдонимы относительны файлу
конфигурации; вложенная `.luaurc` может переопределять родительские. Импорт не даёт полномочий.

Вычисление входного файла не может выполнять эффекты Scope или приостанавливаться.
Модуль, загруженный внутри живого обратного вызова, использует его полномочия и бюджет.
Sequence может приостановиться при инициализации импортируемого модуля, но загрузка
входного файла и вызовы Behavior по-прежнему не могут ждать. Предпочитайте экспорт функций
скрытой асинхронной работе при инициализации модуля.

Используйте ту же структуру исходников и конфигурацию с luau-lsp. Декларации типов описывают
API приложения-хоста; явные аннотации контекста и состояния помогают серверу языка понимать
внедрённые параметры. Они не устанавливают привязки. Автоматического наблюдения за файлами
или переноса состояния нет: успешная замена создаёт новые глобальные значения, `self` и кэш.

## Справочные материалы репозитория

Перед адаптацией фрагментов изучите [полный код хоста переключателя (English)][switch],
[настройку fixture (English)][fixture] и [контракт runtime (English)][runtime].

[switch]:
  <https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/src/scripting.rs>
[fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
[runtime]: https://github.com/kedra-engine/kedra/blob/main/crates/kedra_script/README.md
