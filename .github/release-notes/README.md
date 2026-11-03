# 发布说明

每个版本一个 `<tag>.md`（如 `v2.0.0.md`），与版本号改动一起提交、随 tag 的代码进入发布流水线：
`.github/workflows/release.yml` 的 create-release 作业直接把它作为 GitHub Release 的正文。

没有对应文件时，流水线按上一个 `v*` tag 以来的提交标题生成一份占位说明，并在运行日志里给出警告；
这种情况需要在发布后用 `gh release edit <tag> --notes-file <文件>` 补写。
