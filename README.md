# Aidoku Sources

此仓库托管了可通过 [Aidoku](https://aidoku.app/) 应用程序直接安装的公共源。

## 使用

在 Aidoku 中添加源地址：`https://shupian35.github.io/aidoku-sources/index.min.json`

或使用 `aidoku serve` 命令启动本地服务器。

## 源

| 名称 | 状态 |
| --- | --- |
| [肉漫屋](https://rouman5.com/) | ✅ |
| [鸟鸟韩漫](https://nnhm7.com/) | ✅ |

## 开发

### 前置要求

- Rust toolchain
- [aidoku-cli](https://github.com/Aidoku/aidoku-cli)

### aidoku-cli 命令

```
Usage: aidoku <COMMAND>

Commands:
  package  Build and package a source
  build    Build a source list
  serve    Serve a source on the local network
  verify   Verify a source is ready to be published
  init     Initialize a new source
  help     Print this message or the help of a given subcommand
```

### 项目结构

```
aidoku-sources/
├── src/rust/
│   └── zh.roumanwu/          # 单个源
│       ├── .cargo/
│       ├── res/              # 资源文件 (source.json, icon.png)
│       ├── src/              # Rust 源码
│       ├── Cargo.toml
│       └── build.sh
├── public/                   # 生成的发布目录
│   ├── index.json
│   ├── index.min.json
│   ├── sources/              # .aix 包
│   └── icons/                # 源图标
├── build.ps1                 # Windows 构建脚本
└── build.sh                  # Linux/Mac 构建脚本
```

### 构建单个源

```bash
# 进入源目录
cd sources/zh.roumanwu

# 构建并打包
aidoku package

# 生成 package.aix
```

### 构建源列表

```bash
# 从多个 .aix 包构建源列表
aidoku build -o public -n "Aidoku Sources" sources/zh.roumanwu/package.aix

```

### 启动本地服务器

```bash
# 从 public 目录启动
aidoku serve

# 或从单个包启动
aidoku serve sources/zh.roumanwu/package.aix
```

### 添加新源

1. 在 `src/rust/` 下创建新目录，例如 `src/rust/zh.example/`
2. 运行 `aidoku init` 新建源

## License

MIT
