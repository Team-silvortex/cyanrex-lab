# 0.3.7 module/boundary test run — 2026-09-09

Overall: **failed-unresolved**. 9 Rust regression cases remain failing (4 retained auth, 3 new script, 2 new event deletion). No business fixes, commit, push, deployment restart or host-kernel loads were performed.

- [Module/function/boundary guide](../../../docs/en/testing-network.md) / [中文说明](../../../docs/zh-CN/testing-network.md)
- [Plan](plan.json): all 277 Rust cases assigned once and all 68 OpenAPI operations mapped to responsible modules (mapping is not branch coverage).
- [Result](result.json): case counts, exact commands, failures, test layers, exclusions and cleanup.
- [Source input hashes](source-inputs.json): selected dirty-working-tree inputs; HEAD alone does not identify them.
- Logs are grouped by layer. `development-browser.txt` retains the unsuitable dev-server run; `browser.txt` is the unchanged production-build rerun. `rechecks.txt` is additional confirmation, not additional unique coverage.

Commands/logs replace machine-specific workspace/runtime/temp paths and the disposable database URL with placeholders. Resolve them locally; never point these tests at a deployment database. All data and credentials used by probes are synthetic. No raw deployment configuration or student data is retained.
