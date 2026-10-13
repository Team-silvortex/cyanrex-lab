# module-protocol

共享目录契约定义在 [`../module.schema.json`](../module.schema.json)。直接子目录只有提供
符合契约、且 `name` 与目录名匹配的 `module.json` 时，才可以被发现。

v1 契约覆盖发现元数据和进程内控制状态，有意排除可执行文件、shell 命令、动态库、远程加载
和事件传输。这些能力需要未来的隔离与所有权协议，之后才能成为可信运行时能力。
