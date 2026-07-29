# Aidoku Sources

此仓库托管了可通过 [Aidoku](https://aidoku.app/) 应用程序直接安装的公共源。

## 使用

在 Aidoku 中添加源列表：`http://192.168.31.26:8080/index.min.json`

或使用 `aidoku serve` 命令启动本地服务器。

## 源

| 名称 | 状态 |
| --- | --- |
| [肉漫屋](https://rouman5.com/) | ✅ |

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
│   └── zh.rouman5/          # 单个源
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
cd src/rust/zh.rouman5

# 构建并打包
aidoku package

# 生成 package.aix
```

### 构建源列表

```bash
# 从多个 .aix 包构建源列表
aidoku build -o public -n "Source List" src/rust/zh.rouman5/package.aix

# 或使用脚本构建所有源
./build.ps1    # Windows
./build.sh     # Linux/Mac
```

### 启动本地服务器

```bash
# 从 public 目录启动
aidoku serve

# 或从单个包启动
aidoku serve src/rust/zh.rouman5/package.aix
```

### 添加新源

1. 在 `src/rust/` 下创建新目录，例如 `src/rust/zh.example/`
2. 复制 `zh.rouman5` 的结构作为模板
3. 修改 `res/source.json` 和 `src/lib.rs`
4. 运行 `./build.ps1` 或 `./build.sh` 构建所有源

## License

MIT
