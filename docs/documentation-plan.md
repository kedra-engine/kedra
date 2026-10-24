# Documentation plan

## Status and scope

This plan proposes an information architecture and incremental migration, not the
published manual. See [documentation infrastructure](documentation-infrastructure.md)
for the local site infrastructure and EN/RU entry pages.
Repository policy, ADRs, legal originals and crate-local contracts remain at their
current paths. Eighteen superseded engineering Markdown guides have paired replacements
in the [contract directory](site/en/contributing/contracts/index.md).
No deployment, domain configuration, repository publishing or deletion is
authorized by this plan alone.

The goal is a product-oriented manual that readers can use to understand Kedra
without reading its implementation. Unity Manual is an organizational reference:
progressive explanations, task-oriented instructions, detailed reference pages
and connected topics, not a source of text, screenshots or copied branding.

## Current manual coverage

| Section | English | Russian | Scope |
| --- | --- | --- | --- |
| Introduction | [Introduction](site/en/manual/introduction/index.md) | [Знакомство](site/ru/manual/introduction/index.md) | Kedra and Bevy, product map, current capabilities and limits |
| Getting started | [Getting started](site/en/manual/getting-started/index.md) | [Начало работы](site/ru/manual/getting-started/index.md) | Build prerequisites, asset preparation, startup and first document |
| Editor | [Editor](site/en/manual/editor/index.md) | [Редактор](site/ru/manual/editor/index.md) | Workspace and read-only panel controls, selection and preview requirements |
| Projects | [Projects](site/en/manual/projects/index.md) | [Проекты](site/ru/manual/projects/index.md) | Project structure, creation/opening, paths and identity |
| Assets | [Assets](site/en/manual/assets/index.md) | [Ассеты](site/ru/manual/assets/index.md) | Source/delivery concepts, processing, formats and diagnosis |
| Scenes and prefabs | [Scenes and prefabs](site/en/manual/scenes/index.md) | [Сцены и префабы](site/ru/manual/scenes/index.md) | Documents, composition, lifecycle and source-editing APIs |
| Rendering and UI | [Rendering and UI](site/en/manual/rendering-ui/index.md) | [Графика и UI](site/ru/manual/rendering-ui/index.md) | Explicit spatial/UI hydration, environment and preview stages, authored UI and styles |
| Programming | [Programming](site/en/manual/programming/index.md) | [Программирование](site/ru/manual/programming/index.md) | Rust host/schema integration, Luau programs, capabilities and lifetime |
| Build and distribution | [Build and distribution](site/en/manual/build-and-distribution/index.md) | [Сборка и распространение](site/ru/manual/build-and-distribution/index.md) | Cargo application configuration, runnable assembly, explicit launch, packages and licenses |
| Automation and diagnostics | [Automation and diagnostics](site/en/manual/automation/index.md) | [Автоматизация и диагностика](site/ru/manual/automation/index.md) | Debug opt-in, connection authority, observations, operations, injected input, captures and finite programs |
| Testing | [Testing](site/en/manual/testing/index.md) | [Тестирование](site/ru/manual/testing/index.md) | Contract ownership, repository checks, native scenarios and visual evidence boundaries |
| Command-line reference | [Command-line reference](site/en/manual/cli/index.md) | [Справочник командной строки](site/ru/manual/cli/index.md) | Task selection, editor/assets, project-local builder and repository/native-test dispatch |
| Glossary | [Glossary](site/en/manual/glossary.md) | [Глоссарий](site/ru/manual/glossary.md) | Shared content, lifecycle, UI, scripting and verification terminology |

The twelve sections contain 49 articles plus the glossary per locale: 50 manual
page pairs, or 51 pairs including the locale landing pages. Five contributor entry pairs,
19 engineering contract pairs and one contract-directory pair, one Rust API guide pair
and ten Luau API reference pairs bring the site to 87 locale pairs. Contributor and API
guides are separate top-level surfaces, not manual chapters 13 and 14. The language-neutral
root chooser is separate.
These sections cover part of the product, including instructions from build setup
to document inspection. Engineering contracts remain the source of detailed requirements;
the reader journey still needs end-to-end testing with native input and frames.
Other planned manual topics need their own pages. The migration map identifies
retired guides and the sources that remain authoritative.

## Current contributor and API coverage

The paired [contributor guide](site/en/contributing/index.md) /
[руководство участника](site/ru/contributing/index.md) covers setup navigation, the
codebase map, architecture and changes, tests and review, and documentation contributions.
It links authoritative engineering policy rather than replacing `CONTRIBUTING.md`,
`AGENTS.md`, ADRs, legal originals or crate-local contracts.

The paired [engineering contract directory](site/en/contributing/contracts/index.md) /
[каталог инженерных контрактов](site/ru/contributing/contracts/index.md) groups contracts
by application/content lifecycle, editor panels, windows/docking,
and verification/design provenance. Its separate sidebar keeps detailed contracts out of
the contributor getting-started list. Manual recipes and API lookup link to these owners
without creating competing specifications.

The paired [API guide](site/en/api/index.md) /
[руководство API](site/ru/api/index.md) explains the opt-in local Cargo/rustdoc build.
Only the complete `build:api` output exposes its generated English workspace library
list. It is not a publicly hosted API service, release snapshot or all-target/native
certification. The ordinary `build` remains Rust-free.

The [Luau reference](site/en/api/luau/index.md) /
[справочник Luau](site/ru/api/luau/index.md) is included in the ordinary build and
covers game scripting and live-object operations, editor diagnostics (not plugins),
application automation, test fixtures and native Rust integration. Runtime, conventions
and implementation limits have shared reference pages. The ten pairs retain the
six original Luau page URLs. The reference preserves explicit admission, debug/release
boundaries and current implementation caveats; it does not establish a universal game
SDK, automatic project execution or Play. These EN/RU pages share site search with
the manual and contributor guides; generated rustdoc retains separate English search.

## Remaining manual priorities

The remaining documentation work concerns existing mechanisms, not additional product features:

- Add one bounded source-change experiment on a complete private project copy. Preserve
  identities, prepare fresh matching deliveries and distinguish source instructions from
  demonstrated native input/frame acceptance.
- Verify the reader journey with actual native input and frames on the claimed setup;
  paired instructions and documentation checks do not establish native acceptance.
- Decide hosting/publication separately and retire further legacy documents only after their
  contracts, provenance and incoming links have reviewed replacement homes.
- Consolidate detailed duplicated presentation limits under clear documentation owners
  where useful, retaining operational warnings, provenance and existing contracts.

## Documentation surfaces

| Surface | Audience and responsibility |
| --- | --- |
| Manual | People learning Kedra and using its existing workflows |
| API reference | Rust and Luau developers needing exact interfaces and host/lifecycle contracts |
| Contributing | Contributors changing Kedra, its tools or documentation |
| Roadmap | Readers evaluating explicitly planned capabilities |
| ADRs | Readers investigating architectural decisions and their rationale |

The manual is the main entry point. Crate names are implementation links, not its
primary navigation. Each surface links to the others without duplicating their
authoritative details. Rustdoc remains English initially and is linked from both
manual languages. Existing crate READMEs retain their local contracts.

## Proposed manual contents

Use this writing backlog to plan pages. Each child topic becomes a page only when
it has useful, verified content; do not publish empty placeholders. English slugs
remain identical across locales. Russian labels below define intended navigation,
not translated descriptions of unimplemented behavior.

### Introduction / Знакомство

- What is Kedra? / Что такое Kedra?
- Kedra and Bevy / Kedra и Bevy
- Product map: editor, runtime and tools / Карта продукта: редактор, runtime и инструменты
- Current capabilities and limitations / Текущие возможности и ограничения

### Getting started / Начало работы

- Prerequisites and building from source / Подготовка и сборка из исходников
- Preparing editor assets and launching / Подготовка ресурсов и запуск редактора
- Creating and opening a project / Создание и открытие проекта
- First walkthrough: open and inspect a document / Первый маршрут: открыть и изучить документ

### Core concepts / Основные понятия

- Projects, source catalogs and logical paths / Проекты, каталоги источников и логические пути
- Asset sources, identities and deliveries / Исходники ассетов, идентичность и поставки ресурсов
- Scenes, nodes and components / Сцены, узлы и компоненты
- Prefabs, instances and composition / Префабы, экземпляры и композиция
- Source documents and runtime projections / Исходные документы и runtime-проекции
- Authoring, Save and execution boundaries / Границы авторинга, сохранения и выполнения

### Editor / Редактор

- Workspace, docking and floating windows / Рабочее пространство, docking и отдельные окна
- Asset Browser: folders, search and opening / Asset Browser: папки, поиск и открытие
- Hierarchy: navigation and selection / Hierarchy: навигация и выбор
- Scene View: supported previews and resource readiness /
  Scene View: доступные превью и готовность ресурсов
- Scene View: camera controls and selection / Scene View: управление камерой и выбор
- Inspector: supported presentations and read-only fields /
  Inspector: представления и поля только для чтения
- Shared selection and document sessions / Общий выбор и сеансы документов

### Assets / Ассеты

- Source registration and asset identity / Регистрация источников и идентичность ассетов
- Processing, verifying and consuming deliveries /
  Обработка, проверка и использование поставок ресурсов
- Supported image and model profiles / Поддерживаемые профили изображений и моделей
- Missing, stale and rejected resources / Отсутствующие, устаревшие и отклонённые ресурсы

### Scenes and prefabs / Сцены и префабы

- Document structure and references / Структура документов и ссылки
- Composition, instances and overrides / Композиция, экземпляры и переопределения
- Explicit startup scenes and lifecycle / Явные стартовые сцены и жизненный цикл
- Rust source-editing APIs and Save protocol / Rust API изменения исходников и протокол сохранения
- Source format reference / Справочник формата исходников

Source-editing APIs must be distinguished from editor capabilities. Do not write
instructions for visual editing, editor Save or Play until those paths exist.

### Rendering and UI / Графика и пользовательский интерфейс

- Supported spatial projection / Поддерживаемая пространственная проекция
- Environment assets and lighting ownership / Ассеты окружения и владение освещением
- Model-preview stages and editor lighting / Сцены предпросмотра моделей и освещение редактора
- Authored UI and shared controls / UI из исходных документов и общие элементы управления
- Layout, styling and native window integration / Компоновка, стили и интеграция с нативными окнами

Describe Kedra's supported integration profile, linking to Bevy for upstream
rendering and ECS fundamentals. Do not imply support for every Bevy feature.

### Programming / Программирование

- Rust integration: components, systems and plugins /
  Интеграция с Rust: компоненты, системы и плагины
- Host-owned capabilities and initialization /
  Возможности и инициализация под управлением приложения
- Luau behaviors, sequences and modules / Поведения, последовательности и модули Luau
- Script scopes, lifetime and diagnostics / Области доступа, жизненный цикл и диагностика скриптов

### Build, automation and reference / Сборка, автоматизация и справочники

- Standalone application builds and explicit launch /
  Сборка отдельных приложений и явный запуск
- Packaging, resources and license obligations / Упаковка, ресурсы и лицензионные обязательства
- Opt-in application diagnostics and automation /
  Явно включаемая диагностика и автоматизация приложений
- Native application test workflows / Проверка нативных приложений
- CLI and project configuration reference / Справочник CLI и конфигурации проекта
- Troubleshooting by symptom / Решение проблем по симптомам
- Glossary / Глоссарий

## Contributing contents

The separate English/Russian contributor navigation exposes five entry pages: contribution
workflow, codebase map, architecture and changes, tests and review, and documentation,
plus the grouped engineering contract directory and its 19 substantive contract pairs.
They connect repository setup, subsystem ownership, the package-layer DAG, lifecycle
and authority rules, test ownership and native acceptance to the existing authoritative
quality/security, CI, provenance, release and optional search contracts.

ADRs and legal/provenance originals remain at their stable paths and in their
original language. Both locale trees can link to them with an explicit English
label. Translations must not replace license texts or authoritative provenance.

## Language and page contracts

- English is canonical; Russian is the second documentation language.
- Use the dedicated site source paths `docs/site/en/manual/`,
  `docs/site/ru/manual/`, `docs/site/en/contributing/`, `docs/site/ru/contributing/`,
  `docs/site/en/api/` and `docs/site/ru/api/`.
- Publish corresponding pages under `/en/` and `/ru/` with matching English slugs.
  The root entry must offer both languages without requiring server redirects.
- Switching languages opens the corresponding page. Missing translations must
  have an explicit English fallback, not silently redirect to an unrelated home.
- Translate navigation, search controls, descriptions and explanatory text. Keep
  identifiers, API names, commands and paths unchanged. Use actual editor labels
  in instructions, for example "панель Hierarchy".
- Maintain paired EN/RU glossaries. Introduce a translated technical term with
  its English equivalent where useful; use the chosen term consistently.
- Share screenshots and diagrams where possible, with localized captions and
  alt text. Images must reflect the documented implementation and retain required
  provenance. Design targets are not evidence of implemented UI.
- Deliver the first reader journey in both languages before calling it complete.
  Subsequent manual changes update both versions in the same reviewed change.
- Machine translation may produce drafts, but publication requires factual and
  linguistic review. Never require a private translation or AI service in CI.
- Track untranslated or outdated pages explicitly in the migration inventory;
  do not label a stale translation as current. Start with review of paired files,
  not a custom translation management framework.

Each substantive topic explains purpose, prerequisites, behavior, a concrete
example, relevant failures/limitations and related topics/API. Separate conceptual
explanations, task instructions and detailed reference when readers benefit.
Describe implementation and native verification limits separately. Commands and
examples must be checked against current sources before publication.

## Migration map

### Completed engineering migrations

The following 18 original Markdown guides are retired. Use the canonical EN/RU
contracts for their requirements, limitations, provenance and operational recipes.
Linked Manual pages provide explanations and task instructions. Native input/frames,
hosted releases, physical platforms and public deployment need separate verification.

| Retired original | Canonical engineering owner |
| --- | --- |
| `docs/startup.md` | [Startup](site/en/contributing/contracts/startup.md) |
| `docs/scenes.md` | [Scene runtime](site/en/contributing/contracts/scene-runtime.md); [source authoring](site/en/contributing/contracts/source-authoring.md) |
| `docs/automation.md` | [Automation](site/en/contributing/contracts/automation.md) |
| `docs/testing.md` | [Testing](site/en/contributing/contracts/testing.md) |
| `docs/distribution.md` | [Distribution](site/en/contributing/contracts/distribution.md) |
| `docs/ui/source-documents.md` | [Source documents](site/en/contributing/contracts/editor/source-documents.md) |
| `docs/ui/asset-browser.md` | [Asset Browser](site/en/contributing/contracts/editor/asset-browser.md) |
| `docs/ui/hierarchy.md` | [Hierarchy](site/en/contributing/contracts/editor/hierarchy.md) |
| `docs/ui/inspector.md` | [Inspector](site/en/contributing/contracts/editor/inspector.md) |
| `docs/ui/scene-view.md` | [Scene View](site/en/contributing/contracts/editor/scene-view.md) |
| `docs/ui/scene-view-controls-design.md` | [Scene View controls](site/en/contributing/contracts/editor/scene-view-controls.md) |
| `docs/ui/docking-implementation.md` | [Docking](site/en/contributing/contracts/ui/docking.md) |
| `docs/ui/window-chrome.md` | [Window chrome](site/en/contributing/contracts/ui/window-chrome.md) |
| `docs/ui/docking-tests.md` | [Native docking tests](site/en/contributing/contracts/ui/native-docking-tests.md) |
| `docs/ui/visual-acceptance.md` | [Visual verification](site/en/contributing/contracts/ui/visual-verification.md) |
| `docs/ui/ui-corpus.md` | [UI corpus](site/en/contributing/contracts/ui/ui-corpus.md) |
| `docs/ui/figma.md` | [Design provenance](site/en/contributing/contracts/ui/design-provenance.md) |
| `docs/ui/startup-layout-lock.md` | [Startup visual contract](site/en/contributing/contracts/ui/startup-visual.md) |

### Retained authorities and remaining work

These sources remain current authorities or inputs for future explanatory writing, not
approved whole-file retirement targets. Read original sources and verify current claims
before proposing any further migration.

| Existing source | Proposed destination and treatment |
| --- | --- |
| `README.md` | Retain a concise repository entry; use product/status material for Introduction |
| `CONTRIBUTING.md`, `AGENTS.md` | Retain repository entry/rules; link contributor guides rather than duplicate policy |
| `docs/architecture.md` | Contributor system map and architecture contracts; extract only conceptual explanations for Manual |
| `docs/ui/docking/figma/README.md` | Retain adjacent manifest interpretation/provenance warning; its links target the paired corpus/native-test contracts |
| `docs/ui/` JSON, corpus, policies, manifests and source exports | Retain exact source paths, identities, reviewed approval and licensing authority; Markdown retirement does not move these artifacts |
| `docs/quality.md`, `docs/ci.md` | Retain merge policy, CI and failure-diagnosis authority |
| `docs/hosting.md` | Contributor repository administration and publication boundaries |
| `docs/third-party.md`, `docs/licenses/` | Retain legal/provenance authority; link from packaging and contributor guides |
| `docs/ai-search.md` | Contributor optional repository search; keep private services outside required checks |
| `docs/roadmap.md` | Retain canonical roadmap, linked separately from working-feature instructions |
| `docs/decisions/` | Retain ADR identities, status, rationale and incoming links; no wholesale deletion |
| `crates/*/README.md`, Rust API docs | Retain local contracts/API; use as evidence for Manual without duplicating exact API reference |
| `tests/fixtures/test_project/README.md` | Retain fixture-local ownership/setup; link verified examples from relevant guides |

The product map, reader route and glossary have manual pages. Remaining topics need
explanations, broader cross-system examples and deeper symptom-oriented troubleshooting;
links to engineering contracts alone are not enough.

For each source selected for migration, record its destination pages, remaining
unmigrated contracts and EN/RU review status in a focused checklist. Delete the
source only when all required content has an authoritative home, both required
translations are reviewed and incoming links/anchors have been updated. Preserve
stable public URLs through redirects when a site already exposes them. Avoid
bulk moves, duplicate authoritative descriptions and an unmaintained archive.

## Site implementation proposal

The proposed site implementation uses VitePress as the candidate static-site
generator. Markdown authoring, built-in locales, hierarchical navigation and local
search fit this scope without a custom documentation engine. Its MIT license and
Node tooling are compatible in principle; before adding dependencies, verify the
selected stable release, maintenance, Node requirements, transitive licenses and
security advisories. Pin dependencies and commit the package lock. Do not use a
prerelease merely because upstream main does.

Keep configuration under `docs/.vitepress/`, locale content under `docs/site/en/`
and `docs/site/ru/`, and shared media under `docs/site/public/`. The dedicated source
root excludes legacy documents from publication. Generate output under `target/`.
Do not expose the entire legacy `docs/` tree as reviewed manual content.
Use the maintained default theme, not custom components or a copied Unity skin.
Validate language switching, keyboard navigation, narrow layouts and EN/RU search
with real queries, including Russian terminology and unchanged API names.

Provide reproducible local build and CI validation using maintained tools and
existing repository orchestration. Check both locale builds, internal links/anchors,
Markdown and required page pairs without weakening existing gates. Establish which
examples are runnable and validate them through their production owners. Building
documentation must not require an editor window, credentials or private services.
Hosting, deployment permissions and API-reference publication are separate setup
decisions; do not publish automatically during the initial implementation.

## Delivery sequence and completion criteria

1. **Structure:** review the contents list and migration map; choose the first
   source pages and identify the owning production modules to verify.
2. **Local site:** provide the pinned toolchain/configuration, localized navigation,
   search, shared media layout and a non-published build. Keep existing docs intact.
3. **First journey:** provide reviewed EN/RU content explaining Kedra and Bevy,
   prerequisites, asset preparation, startup, project opening, panel navigation and
   opening/inspecting a supported scene or prefab. Explain document selection,
   delivery readiness, read-only behavior and why a preview is not Play.
4. **Subsystem depth:** cover Assets and Scenes first, then Rendering/UI,
   Programming, builds/automation and contributor guides. Reuse verified examples
   and include troubleshooting alongside the relevant workflows.
5. **Incremental retirement:** review each source checklist, update links, then
   remove only superseded material. Retain ADRs, licenses and local crate contracts.
6. **Publication:** separately approve hosting and deployment. Introduce release
   snapshots when real releases need them; do not invent a version compatibility
   promise for today's unstable APIs and formats.

The first journey is complete when a reader can follow it from repository setup
to inspecting a document without undocumented mandatory steps, in both languages,
with working navigation/search and explicit capability limits. Any claimed native
workflow needs actual input and frames; a site build cannot certify editor behavior.
Writing the remaining manual is separate work; site scaffolding does not complete it.
