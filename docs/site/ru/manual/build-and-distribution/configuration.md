# Конфигурация приложения

Kedra читает настройки отдельной сборки из `Cargo.toml`, а не из `kedra.toml`.
У выбранного пакета должна быть ровно одна бинарная цель, метаданные нативного пакета
и метаданные поставки Kedra. Cargo определяет имя пакета, версию и идентичность бинарной цели.

Отличайте это от [создания пустого проекта редактора](../projects/create-open.md).
Перед использованием метаданных ниже изучите
[настройку приложения-хоста](../programming/index.md),
[подготовку ассетов](../assets/pipeline.md) и [требования сборки/запуска](build-and-run.md).

## Локальный launcher проекта

В этом репозитории `.cargo/config.toml` содержит:

```toml
[alias]
xtask = "run --locked --package xtask --target-dir target/xtask-bootstrap --"
kedra = "run --locked --package kedra-build-tool --target-dir target/kedra-build-bootstrap --"
```

Launcher в `tools/kedra` зависит от инструментального crate `kedra_build`. Его точка входа
целиком передаёт нативные аргументы, не преобразуя их в строки UTF-8:

```rust
fn main() -> std::process::ExitCode {
    kedra_build::run_cli(std::env::args_os())
}
```

Держите launcher отдельно от runtime-зависимостей приложения. Изолированный каталог
bootstrap-сборки не даёт сборке workspace перезаписать работающий инструмент в Windows.
Сохраняйте существующий alias `xtask`: у него другая задача. Независимому проекту нужны
собственная проверенная конфигурация launcher/зависимостей и lockfile; копирование alias
из корня само по себе не устанавливает Kedra и не создаёт приложение.

## Реальный пример метаданных

Ниже приведена текущая конфигурация редактора из `apps/kedra/Cargo.toml`:

```toml
[package.metadata.packager]
productName = "Kedra"
identifier = "dev.kedra.editor"
description = "Open-source game development environment built on Bevy"
publisher = "Kedra contributors"

[package.metadata.packager.macos]
minimumSystemVersion = "15.0"

[package.metadata.kedra.distribution]
assets = "../../assets/editor"
profile = "editor-desktop"
namespace = "editor"
icon = "../../assets/editor/startup/project-mark-compact.svg"
notices = ["../../LICENSE-MIT", "../../LICENSE-APACHE", "../../docs/third-party.md"]
license-config = "../../about.toml"
```

Эти относительные пути отсчитываются от каталога пакета `apps/kedra`, а не от рабочего
каталога терминала. Компоненты родительского пути здесь допустимы, если разрешённый путь
остаётся внутри workspace Cargo, в том числе после разрешения файловых ссылок.

Не копируйте идентификатор, namespace и уведомления редактора в качестве собственной
идентичности и юридической описи приложения. Выберите подходящие значения и изучите
фактическое содержимое поставки.

## Справочник полей

| Настройка | Требуемое значение |
| --- | --- |
| Cargo `package.name` / `package.version` | Идентичность приложения в результатах сборки и упаковки |
| `packager.productName` | Непустое отображаемое имя без разделителей пути и управляющих символов |
| `packager.identifier` | Непустой нативный идентификатор из ASCII-букв, цифр, `.` или `-` |
| `packager.version` | Отсутствует либо точно совпадает с версией Cargo |
| `distribution.assets` | Существующий каталог исходных ассетов внутри workspace |
| `distribution.profile` | Профиль обработки/поставки, например `editor-desktop` или `game-desktop` |
| `distribution.namespace` | Логический namespace источника для создаваемой поставки ресурсов |
| `distribution.icon` | Существующий самодостаточный SVG |
| `distribution.notices` | Список существующих файлов лицензий/уведомлений внутри workspace |
| `distribution.license-config` | Необязательная конфигурация cargo-about относительно пакета |

Все поля distribution, кроме `license-config`, обязательны. Неизвестные поля отклоняются.
Уведомления копируются в `licenses/` под исходными именами файлов; используйте разные имена,
чтобы избежать коллизий. Общий инструмент определяет пути вывода, форматы, создаваемые
значки и размещение ресурсов; задать другой процесс компоновки через эти поля нельзя.

Непустой `packager.resources` отклоняется. Задавайте поддерживаемые входы через `assets`
и `notices`; произвольные дополнительные файлы конфигурации/ресурсов пока не поддерживаются.
Дополнительные AppImage `libs`, `bins`, `files` и плагины linuxdeploy также отклоняются
до компиляции: обработка их нативного происхождения и исходников не реализована.

## Подготавливайте исходники осознанно

Храните существующие `.meta` sidecar-файлы вместе с ассетами. Сборка и упаковка обрабатывают
исходники, но никогда не инициализируют идентификаторы неявно. Только для собственного
каталога исходников доступна явная инициализация:

```sh
cargo kedra assets init assets
```

Эта операция авторинга добавляет недостающие sidecar-файлы и сохраняет существующие
метаданные. Это не обязательный этап каждой сборки. Не запускайте её над проверочными
примерами репозитория просто ради успешного выполнения примера.
Подробнее: [Ассеты](../assets/index.md).

## Как понимать проверочные проекты

Это фикстуры для приёмки, а не шаблоны приложений или установленный SDK:

- `tests/fixtures/distribution` использует версии-заполнители зависимостей из реестра для
  изолированной проверочной копии с явно выбранными локальными или закреплёнными
  Git-зависимостями. [Контракт фикстуры (English)][distribution-fixture] описывает
  подготовку и изоляцию runtime.
- У `tests/fixtures/test_project` нет метаданных `packager`/`distribution`; это не готовый
  пример для `cargo kedra package`. [Контракт фикстуры (English)][native-fixture] и
  [нативное руководство (English)][native-guide] описывают явные поставки ресурсов
  редактора/игры и подготовку.

Корректность проекта редактора и конфигурация распространения — разные контракты.

Далее: [сборка и запуск](build-and-run.md), затем [упаковка](packaging-and-licenses.md).

[distribution-fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/distribution/README.md
[native-fixture]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/README.md
[native-guide]: https://github.com/kedra-engine/kedra/blob/main/tests/fixtures/test_project/native.md
