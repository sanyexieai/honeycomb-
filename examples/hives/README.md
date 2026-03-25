# Example Hives

当前提供两个最小样例：

- `summarizer/`：Python 脚本 worker
- `summarizer_bin/`：目录内独立二进制 worker

可以用下面的命令验证目录结构：

```bash
cargo run -- validate examples/hives/summarizer
cargo run -- validate examples/hives/summarizer_bin
```
