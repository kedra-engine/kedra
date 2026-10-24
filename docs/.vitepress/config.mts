import { defineConfig } from 'vitepress'
import license from 'rollup-plugin-license'
import { fileURLToPath } from 'node:url'
import { accessSync, readFileSync } from 'node:fs'

// Only the explicit complete build supplies generated library links to the entry pages.
const apiCrates = process.env.KEDRA_DOCS_API === '1'
  ? JSON.parse(readFileSync(new URL('../../target/documentation/api-manifest.json',
    import.meta.url), 'utf8'))
  : []
for (const crate of apiCrates) {
  accessSync(new URL(`../../target/documentation/cargo/doc/${crate.name}/index.html`,
    import.meta.url))
}

export default defineConfig({
  title: 'Kedra Documentation',
  lang: 'en',
  description: 'Kedra documentation / Документация Kedra',
  srcDir: 'site',
  outDir: '../target/documentation/site',
  cacheDir: '../target/documentation/cache',
  cleanUrls: false,
  transformPageData(page) {
    if (/^(en|ru)\/api\/index\.md$/.test(page.relativePath)) {
      page.frontmatter.apiCrates = apiCrates
    }
  },
  // RON examples share Rust token highlighting; this does not validate their schema.
  markdown: { languageAlias: { ron: 'rust' } },
  vite: {
    plugins: [license({
      thirdParty: {
        includePrivate: false,
        output: fileURLToPath(new URL('../../target/documentation/site/third-party-notices.txt', import.meta.url)),
        allow: {
          test: '(MIT OR Apache-2.0 OR BSD-2-Clause OR BSD-3-Clause OR ISC OR CC0-1.0)',
          failOnUnlicensed: true,
          failOnViolation: true,
        },
      },
    })],
  },
  locales: {
    en: {
      label: 'English',
      lang: 'en',
      description: 'Documentation for Kedra, a game development environment built on Bevy.',
      themeConfig: {
        nav: [
          { text: 'Documentation', link: '/en/' },
          { text: 'Manual', link: '/en/manual/introduction/' },
          { text: 'Contributing', link: '/en/contributing/' },
          { text: 'API reference', link: '/en/api/' },
          { text: 'Third-party notices', link: '/third-party-notices.txt' },
        ],
        sidebar: {
          '/en/contributing/contracts/': [
            { text: 'Engineering contracts', items: [
              { text: 'Choose a contract owner', link: '/en/contributing/contracts/' },
              { text: 'Codebase map', link: '/en/contributing/codebase-map' },
            ] },
            { text: 'Application and content lifecycle', items: [
              { text: 'Startup', link: '/en/contributing/contracts/startup' },
              { text: 'Scene runtime', link: '/en/contributing/contracts/scene-runtime' },
              { text: 'Source authoring', link: '/en/contributing/contracts/source-authoring' },
              { text: 'Application automation', link: '/en/contributing/contracts/automation' },
              { text: 'Distribution', link: '/en/contributing/contracts/distribution' },
            ] },
            { text: 'Editor panels', items: [
              { text: 'Source documents', link: '/en/contributing/contracts/editor/source-documents' },
              { text: 'Asset Browser', link: '/en/contributing/contracts/editor/asset-browser' },
              { text: 'Hierarchy', link: '/en/contributing/contracts/editor/hierarchy' },
              { text: 'Inspector', link: '/en/contributing/contracts/editor/inspector' },
              { text: 'Scene View', link: '/en/contributing/contracts/editor/scene-view' },
              { text: 'Scene View controls', link: '/en/contributing/contracts/editor/scene-view-controls' },
            ] },
            { text: 'Windows and docking', items: [
              { text: 'Docking', link: '/en/contributing/contracts/ui/docking' },
              { text: 'Window chrome', link: '/en/contributing/contracts/ui/window-chrome' },
            ] },
            { text: 'Verification and design provenance', items: [
              { text: 'Testing', link: '/en/contributing/contracts/testing' },
              { text: 'Native docking tests', link: '/en/contributing/contracts/ui/native-docking-tests' },
              { text: 'Visual verification', link: '/en/contributing/contracts/ui/visual-verification' },
              { text: 'UI corpus', link: '/en/contributing/contracts/ui/ui-corpus' },
              { text: 'Design provenance', link: '/en/contributing/contracts/ui/design-provenance' },
              { text: 'Startup visual contract', link: '/en/contributing/contracts/ui/startup-visual' },
            ] },
            { text: 'Related surfaces', items: [
              { text: 'Contributing', link: '/en/contributing/' },
              { text: 'Manual', link: '/en/manual/introduction/' },
              { text: 'API reference', link: '/en/api/' },
            ] },
          ],
          '/en/contributing/': [
            { text: 'Contributing', items: [
              { text: 'Start contributing', link: '/en/contributing/' },
              { text: 'Codebase map', link: '/en/contributing/codebase-map' },
              { text: 'Engineering contracts', link: '/en/contributing/contracts/' },
              { text: 'Architecture and changes', link: '/en/contributing/architecture-and-changes' },
              { text: 'Tests and review', link: '/en/contributing/tests-and-review' },
              { text: 'Documentation', link: '/en/contributing/documentation' },
            ] },
            { text: 'Related surfaces', items: [
              { text: 'Manual', link: '/en/manual/introduction/' },
              { text: 'API reference', link: '/en/api/' },
            ] },
          ],
          '/en/api/': [
            { text: 'API reference', items: [
              { text: 'Rust guide and generated reference', link: '/en/api/' },
               { text: 'Luau: choose a task', link: '/en/api/luau/' },
            ] },
             { text: 'Game scripting', items: [
               { text: 'Game scripts and authorization', link: '/en/api/luau/game-host' },
               { text: 'Live-object operations', link: '/en/api/luau/bevy' },
             ] },
             { text: 'Automation, diagnostics and hosts', items: [
               { text: 'Editor diagnostics (not plugins)', link: '/en/api/luau/editor' },
               { text: 'Application automation', link: '/en/api/luau/application' },
               { text: 'Test fixtures', link: '/en/api/luau/testing' },
               { text: 'Native Rust integration', link: '/en/api/luau/integration' },
             ] },
             { text: 'Shared Luau foundation', items: [
               { text: 'Runtime', link: '/en/api/luau/runtime' },
               { text: 'Conventions and failures', link: '/en/api/luau/conventions' },
               { text: 'Current qualifications', link: '/en/api/luau/limitations' },
            ] },
            { text: 'Related surfaces', items: [
              { text: 'Programming manual', link: '/en/manual/programming/' },
              { text: 'Contributing', link: '/en/contributing/' },
            ] },
          ],
          '/en/': [
          { text: 'Documentation', items: [{ text: 'Overview', link: '/en/' }] },
          {
            text: 'Introduction',
            items: [
              { text: 'What is Kedra?', link: '/en/manual/introduction/' },
              { text: 'Kedra and Bevy', link: '/en/manual/introduction/kedra-and-bevy' },
              { text: 'Product map', link: '/en/manual/introduction/product-map' },
              { text: 'Current capabilities and limitations', link: '/en/manual/introduction/capabilities' },
            ],
          },
          {
            text: 'Getting started',
            items: [
              { text: 'Prerequisites and source setup', link: '/en/manual/getting-started/' },
              { text: 'Prepare resources and launch', link: '/en/manual/getting-started/launch-editor' },
              { text: 'Open and inspect your first document', link: '/en/manual/getting-started/first-document' },
            ],
          },
          {
            text: 'Editor',
            items: [
              { text: 'Editor workspace', link: '/en/manual/editor/' },
              { text: 'Asset Browser', link: '/en/manual/editor/asset-browser' },
              { text: 'Hierarchy', link: '/en/manual/editor/hierarchy' },
              { text: 'Scene View', link: '/en/manual/editor/scene-view' },
              { text: 'Inspector', link: '/en/manual/editor/inspector' },
            ],
          },
          {
            text: 'Projects',
            items: [
              { text: 'Projects', link: '/en/manual/projects/' },
              { text: 'Create or open a project', link: '/en/manual/projects/create-open' },
              { text: 'Paths and identity', link: '/en/manual/projects/paths-and-identity' },
            ],
          },
          {
            text: 'Assets',
            items: [
              { text: 'Assets', link: '/en/manual/assets/' },
              { text: 'Process and verify assets', link: '/en/manual/assets/pipeline' },
              { text: 'Supported formats and processing settings', link: '/en/manual/assets/supported-formats' },
              { text: 'Troubleshoot asset delivery', link: '/en/manual/assets/troubleshooting' },
            ],
          },
          {
            text: 'Scenes and prefabs',
            items: [
              { text: 'Scenes and prefabs', link: '/en/manual/scenes/' },
              { text: 'Composition and overrides', link: '/en/manual/scenes/composition' },
              { text: 'Loading and lifecycle', link: '/en/manual/scenes/lifecycle' },
               { text: 'Source editing and Save (Rust tool hosts)', link: '/en/manual/scenes/source-editing' },
            ],
          },
          {
            text: 'Programming',
            items: [
              { text: 'Programming', link: '/en/manual/programming/' },
              { text: 'Rust components and host integration', link: '/en/manual/programming/rust-components' },
              { text: 'Luau programs and modules', link: '/en/manual/programming/luau' },
              { text: 'Lifetime and capabilities', link: '/en/manual/programming/lifetime-and-capabilities' },
            ],
          },
          {
            text: 'Graphics and UI',
            items: [
              { text: 'Graphics and UI', link: '/en/manual/rendering-ui/' },
              { text: 'Environments and lighting', link: '/en/manual/rendering-ui/environments-and-lighting' },
              { text: 'Preview stages', link: '/en/manual/rendering-ui/preview-stages' },
              { text: 'Authored UI', link: '/en/manual/rendering-ui/authored-ui' },
              { text: 'Styling and layout', link: '/en/manual/rendering-ui/styling-and-layout' },
            ],
          },
          {
            text: 'Build and distribution',
            items: [
              { text: 'Build and distribution', link: '/en/manual/build-and-distribution/' },
              { text: 'Application configuration', link: '/en/manual/build-and-distribution/configuration' },
              { text: 'Build and run', link: '/en/manual/build-and-distribution/build-and-run' },
              { text: 'Packaging and licenses', link: '/en/manual/build-and-distribution/packaging-and-licenses' },
            ],
          },
          {
            text: 'Automation and diagnostics',
            items: [
              { text: 'Automation and diagnostics', link: '/en/manual/automation/' },
              { text: 'Connect to an application', link: '/en/manual/automation/connecting' },
              { text: 'Observation and operations', link: '/en/manual/automation/observation-and-operations' },
              { text: 'Input and renderer captures', link: '/en/manual/automation/input-and-captures' },
              { text: 'Programs and diagnostics', link: '/en/manual/automation/programs-and-diagnostics' },
            ],
          },
          {
            text: 'Testing',
            items: [
              { text: 'Testing', link: '/en/manual/testing/' },
              { text: 'Repository checks', link: '/en/manual/testing/repository-checks' },
              { text: 'Native scenarios', link: '/en/manual/testing/native-scenarios' },
              { text: 'Visual verification', link: '/en/manual/testing/visual-verification' },
            ],
          },
          {
            text: 'Command-line reference',
            items: [
              { text: 'Choose a command', link: '/en/manual/cli/' },
              { text: 'Editor startup and assets', link: '/en/manual/cli/editor-and-assets' },
              { text: 'Project-local builder', link: '/en/manual/cli/build-tool' },
              { text: 'Repository checks and test dispatch', link: '/en/manual/cli/repository-and-tests' },
            ],
          },
          {
            text: 'Reference',
            items: [{ text: 'Glossary', link: '/en/manual/glossary' }],
          },
          ],
        },
        outline: { label: 'On this page' },
        langMenuLabel: 'Change language',
        returnToTopLabel: 'Return to top',
        sidebarMenuLabel: 'Menu',
        darkModeSwitchLabel: 'Appearance',
      },
    },
    ru: {
      label: 'Русский',
      lang: 'ru',
      description: 'Документация Kedra — среды разработки игр на основе Bevy.',
      themeConfig: {
        nav: [
          { text: 'Документация', link: '/ru/' },
          { text: 'Руководство', link: '/ru/manual/introduction/' },
          { text: 'Участие в разработке', link: '/ru/contributing/' },
          { text: 'Справочник API', link: '/ru/api/' },
          { text: 'Лицензии', link: '/third-party-notices.txt' },
        ],
        sidebar: {
          '/ru/contributing/contracts/': [
            { text: 'Инженерные контракты', items: [
              { text: 'Выбор владельца контракта', link: '/ru/contributing/contracts/' },
              { text: 'Карта кодовой базы', link: '/ru/contributing/codebase-map' },
            ] },
            { text: 'Жизненный цикл приложения и содержимого', items: [
              { text: 'Запуск', link: '/ru/contributing/contracts/startup' },
              { text: 'Runtime сцен', link: '/ru/contributing/contracts/scene-runtime' },
              { text: 'Авторинг исходников', link: '/ru/contributing/contracts/source-authoring' },
              { text: 'Автоматизация приложения', link: '/ru/contributing/contracts/automation' },
              { text: 'Распространение', link: '/ru/contributing/contracts/distribution' },
            ] },
            { text: 'Панели редактора', items: [
              { text: 'Исходные документы', link: '/ru/contributing/contracts/editor/source-documents' },
              { text: 'Asset Browser', link: '/ru/contributing/contracts/editor/asset-browser' },
              { text: 'Hierarchy', link: '/ru/contributing/contracts/editor/hierarchy' },
              { text: 'Inspector', link: '/ru/contributing/contracts/editor/inspector' },
              { text: 'Scene View', link: '/ru/contributing/contracts/editor/scene-view' },
              { text: 'Управление Scene View', link: '/ru/contributing/contracts/editor/scene-view-controls' },
            ] },
            { text: 'Окна и docking', items: [
              { text: 'Docking', link: '/ru/contributing/contracts/ui/docking' },
              { text: 'Заголовки окон', link: '/ru/contributing/contracts/ui/window-chrome' },
            ] },
            { text: 'Проверки и происхождение дизайна', items: [
              { text: 'Тестирование', link: '/ru/contributing/contracts/testing' },
              { text: 'Нативные тесты docking', link: '/ru/contributing/contracts/ui/native-docking-tests' },
              { text: 'Визуальная проверка', link: '/ru/contributing/contracts/ui/visual-verification' },
              { text: 'Корпус UI', link: '/ru/contributing/contracts/ui/ui-corpus' },
              { text: 'Происхождение дизайна', link: '/ru/contributing/contracts/ui/design-provenance' },
              { text: 'Визуальный контракт запуска', link: '/ru/contributing/contracts/ui/startup-visual' },
            ] },
            { text: 'Связанные разделы', items: [
              { text: 'Участие в разработке', link: '/ru/contributing/' },
              { text: 'Руководство', link: '/ru/manual/introduction/' },
              { text: 'Справочник API', link: '/ru/api/' },
            ] },
          ],
          '/ru/contributing/': [
            { text: 'Участие в разработке', items: [
              { text: 'С чего начать', link: '/ru/contributing/' },
              { text: 'Карта кодовой базы', link: '/ru/contributing/codebase-map' },
              { text: 'Инженерные контракты', link: '/ru/contributing/contracts/' },
              { text: 'Архитектура и изменения', link: '/ru/contributing/architecture-and-changes' },
              { text: 'Тесты и ревью', link: '/ru/contributing/tests-and-review' },
              { text: 'Работа с документацией', link: '/ru/contributing/documentation' },
            ] },
            { text: 'Связанные разделы', items: [
              { text: 'Руководство', link: '/ru/manual/introduction/' },
              { text: 'Справочник API', link: '/ru/api/' },
            ] },
          ],
          '/ru/api/': [
            { text: 'Справочник API', items: [
              { text: 'Rust: руководство и сгенерированный справочник', link: '/ru/api/' },
               { text: 'Luau: выберите задачу', link: '/ru/api/luau/' },
            ] },
             { text: 'Игровые скрипты', items: [
               { text: 'Скрипты игры и разрешение выполнения', link: '/ru/api/luau/game-host' },
               { text: 'Операции с живыми объектами', link: '/ru/api/luau/bevy' },
             ] },
             { text: 'Автоматизация, диагностика и хосты', items: [
               { text: 'Диагностика редактора (не плагины)', link: '/ru/api/luau/editor' },
               { text: 'Автоматизация приложения', link: '/ru/api/luau/application' },
               { text: 'Тестовые стенды', link: '/ru/api/luau/testing' },
               { text: 'Нативная интеграция на Rust', link: '/ru/api/luau/integration' },
             ] },
             { text: 'Общая основа Luau', items: [
               { text: 'Runtime', link: '/ru/api/luau/runtime' },
               { text: 'Соглашения и ошибки', link: '/ru/api/luau/conventions' },
               { text: 'Текущие оговорки', link: '/ru/api/luau/limitations' },
            ] },
            { text: 'Связанные разделы', items: [
              { text: 'Руководство по программированию', link: '/ru/manual/programming/' },
              { text: 'Участие в разработке', link: '/ru/contributing/' },
            ] },
          ],
          '/ru/': [
          { text: 'Документация', items: [{ text: 'Обзор', link: '/ru/' }] },
          {
            text: 'Знакомство',
            items: [
              { text: 'Что такое Kedra?', link: '/ru/manual/introduction/' },
              { text: 'Kedra и Bevy', link: '/ru/manual/introduction/kedra-and-bevy' },
              { text: 'Карта продукта', link: '/ru/manual/introduction/product-map' },
              { text: 'Текущие возможности и ограничения', link: '/ru/manual/introduction/capabilities' },
            ],
          },
          {
            text: 'Начало работы',
            items: [
              { text: 'Требования и подготовка исходников', link: '/ru/manual/getting-started/' },
              { text: 'Подготовка ресурсов и запуск', link: '/ru/manual/getting-started/launch-editor' },
              { text: 'Открытие и изучение первого документа', link: '/ru/manual/getting-started/first-document' },
            ],
          },
          {
            text: 'Редактор',
            items: [
              { text: 'Рабочее пространство редактора', link: '/ru/manual/editor/' },
              { text: 'Asset Browser', link: '/ru/manual/editor/asset-browser' },
              { text: 'Hierarchy', link: '/ru/manual/editor/hierarchy' },
              { text: 'Scene View', link: '/ru/manual/editor/scene-view' },
              { text: 'Inspector', link: '/ru/manual/editor/inspector' },
            ],
          },
          {
            text: 'Проекты',
            items: [
              { text: 'Проекты', link: '/ru/manual/projects/' },
              { text: 'Создание и открытие проекта', link: '/ru/manual/projects/create-open' },
              { text: 'Пути и идентичность', link: '/ru/manual/projects/paths-and-identity' },
            ],
          },
          {
            text: 'Ассеты',
            items: [
              { text: 'Ассеты', link: '/ru/manual/assets/' },
              { text: 'Обработка и проверка ассетов', link: '/ru/manual/assets/pipeline' },
              { text: 'Поддерживаемые форматы и настройки обработки', link: '/ru/manual/assets/supported-formats' },
              { text: 'Решение проблем с поставкой ресурсов', link: '/ru/manual/assets/troubleshooting' },
            ],
          },
          {
            text: 'Сцены и префабы',
            items: [
              { text: 'Сцены и префабы', link: '/ru/manual/scenes/' },
              { text: 'Композиция и переопределения', link: '/ru/manual/scenes/composition' },
              { text: 'Загрузка и жизненный цикл', link: '/ru/manual/scenes/lifecycle' },
               { text: 'Изменение исходников и Save (Rust-хосты)', link: '/ru/manual/scenes/source-editing' },
            ],
          },
          {
            text: 'Программирование',
            items: [
              { text: 'Программирование', link: '/ru/manual/programming/' },
              { text: 'Компоненты Rust и приложение-хост', link: '/ru/manual/programming/rust-components' },
              { text: 'Программы и модули Luau', link: '/ru/manual/programming/luau' },
              { text: 'Жизненный цикл и полномочия', link: '/ru/manual/programming/lifetime-and-capabilities' },
            ],
          },
          {
            text: 'Графика и UI',
            items: [
              { text: 'Графика и UI', link: '/ru/manual/rendering-ui/' },
              { text: 'Окружение и освещение', link: '/ru/manual/rendering-ui/environments-and-lighting' },
              { text: 'Сцены предпросмотра', link: '/ru/manual/rendering-ui/preview-stages' },
              { text: 'UI из исходных документов', link: '/ru/manual/rendering-ui/authored-ui' },
              { text: 'Стили и компоновка', link: '/ru/manual/rendering-ui/styling-and-layout' },
            ],
          },
          {
            text: 'Сборка и распространение',
            items: [
              { text: 'Сборка и распространение', link: '/ru/manual/build-and-distribution/' },
              { text: 'Конфигурация приложения', link: '/ru/manual/build-and-distribution/configuration' },
              { text: 'Сборка и запуск', link: '/ru/manual/build-and-distribution/build-and-run' },
              { text: 'Упаковка и лицензии', link: '/ru/manual/build-and-distribution/packaging-and-licenses' },
            ],
          },
          {
            text: 'Автоматизация и диагностика',
            items: [
              { text: 'Автоматизация и диагностика', link: '/ru/manual/automation/' },
              { text: 'Подключение к приложению', link: '/ru/manual/automation/connecting' },
              { text: 'Наблюдение и операции', link: '/ru/manual/automation/observation-and-operations' },
              { text: 'Ввод и захват кадров', link: '/ru/manual/automation/input-and-captures' },
              { text: 'Программы и диагностика', link: '/ru/manual/automation/programs-and-diagnostics' },
            ],
          },
          {
            text: 'Тестирование',
            items: [
              { text: 'Тестирование', link: '/ru/manual/testing/' },
              { text: 'Проверки репозитория', link: '/ru/manual/testing/repository-checks' },
              { text: 'Нативные сценарии', link: '/ru/manual/testing/native-scenarios' },
              { text: 'Визуальная проверка', link: '/ru/manual/testing/visual-verification' },
            ],
          },
          {
            text: 'Справочник командной строки',
            items: [
              { text: 'Выбор команды', link: '/ru/manual/cli/' },
              { text: 'Запуск редактора и ассеты', link: '/ru/manual/cli/editor-and-assets' },
              { text: 'Локальный сборщик проекта', link: '/ru/manual/cli/build-tool' },
              { text: 'Проверки репозитория и запуск тестов', link: '/ru/manual/cli/repository-and-tests' },
            ],
          },
          {
            text: 'Справочные материалы',
            items: [{ text: 'Глоссарий', link: '/ru/manual/glossary' }],
          },
          ],
        },
        outline: { label: 'На этой странице' },
        langMenuLabel: 'Изменить язык',
        skipToContentLabel: 'Перейти к содержимому',
        returnToTopLabel: 'Наверх',
        sidebarMenuLabel: 'Меню',
        darkModeSwitchLabel: 'Оформление',
        lightModeSwitchTitle: 'Включить светлую тему',
        darkModeSwitchTitle: 'Включить тёмную тему',
        docFooter: { prev: 'Предыдущая страница', next: 'Следующая страница' },
        notFound: {
          title: 'СТРАНИЦА НЕ НАЙДЕНА',
          quote: 'Этой страницы нет. Вернитесь к обзору документации.',
          linkLabel: 'Открыть документацию',
          linkText: 'Открыть документацию',
        },
      },
    },
  },
  themeConfig: {
    search: {
      provider: 'local',
      options: {
        locales: {
          ru: {
            translations: {
              button: { buttonText: 'Поиск', buttonAriaLabel: 'Поиск по документации' },
              modal: {
                displayDetails: 'Показать подробности',
                resetButtonTitle: 'Сбросить поиск',
                backButtonTitle: 'Закрыть поиск',
                noResultsText: 'Ничего не найдено',
                footer: { selectText: 'выбрать', navigateText: 'перейти', closeText: 'закрыть' },
              },
            },
          },
        },
      },
    },
    socialLinks: [{ icon: 'github', link: 'https://github.com/kedra-engine/kedra' }],
  },
})
