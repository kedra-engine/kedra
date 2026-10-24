# Engineering contracts

Choose the contract for the behavior you are changing.
Use the [Manual](../../manual/introduction/index.md) to learn and use Kedra;
use the [API guide](../../api/index.md) to look up interfaces.
These contracts explain invariants, authority, failure behavior and verification limits.
Repository policy, ADRs, legal texts and crate/module APIs retain their own authority.

## Application and content lifecycle

| Task | Contract owner |
| --- | --- |
| Change boot, project preparation or surface lifetime | [Startup](startup.md) |
| Change schema admission, compilation, instances or explicit scene startup | [Scene runtime](scene-runtime.md) |
| Change source transactions, history or the Save protocol | [Source authoring](source-authoring.md) |
| Change debug connection, operations, injected input or captures | [Application automation](automation.md) |
| Change assembly, installed resources, packaging or release orchestration | [Distribution](distribution.md) |

## Editor panels

| Task | Contract owner |
| --- | --- |
| Change source admission, asynchronous Open or shared selection | [Source documents](editor/source-documents.md) |
| Change folder listing, cache limits or retained browser presentation | [Asset Browser](editor/asset-browser.md) |
| Change composed-tree navigation and scoped node selection | [Hierarchy](editor/hierarchy.md) |
| Change reflected values, borrowed bindings or retained field views | [Inspector](editor/inspector.md) |
| Change preview delivery binding, projection isolation or native recipients | [Scene View](editor/scene-view.md) |
| Change camera commands, input leases or control geometry | [Scene View controls](editor/scene-view-controls.md) |

## Windows and docking

| Task | Contract owner |
| --- | --- |
| Change measured transactions, native hosts or workspace preferences | [Docking](ui/docking.md) |
| Change borrowed header slots, input regions or native chrome integration | [Window chrome](ui/window-chrome.md) |

## Verification and design provenance

| Task | Contract owner |
| --- | --- |
| Choose test ownership, fixtures and evidence boundaries | [Testing](testing.md) |
| Verify docking through real processes, input and frame fences | [Native docking tests](ui/native-docking-tests.md) |
| Change offline comparison, budgets, exclusions or evidence readiness | [Visual verification](ui/visual-verification.md) |
| Maintain reviewed control sources, typography and corpus policy | [UI corpus](ui/ui-corpus.md) |
| Find design/export identities and licensing authorities | [Design provenance](ui/design-provenance.md) |
| Interpret independent startup geometry and its acceptance limits | [Startup visual contract](ui/startup-visual.md) |

For implementation locations and retained repository policy, use the
[codebase map](../codebase-map.md); for review commands, use
[Tests and review](../tests-and-review.md).
Source inspection, compilation, injected input, physical input and visual comparison prove
different things. A documented contract is not evidence of current native acceptance.
