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

### 项目结构

```
aidoku-sources/
├── src/rust/
│   └── zh.rouman5/          # 单个源
│       ├── .cargo/
│       ├── res/              # 资源文件 (source.json, Icon.png)
│       ├── src/              # Rust 源码
│       ├── Cargo.toml
│       └── build.sh
├── public/                   # 生成的发布目录
│   ├── index.json
│   ├── sources/              # .aix 包
│   └── icons/                # 源图标
├── build.ps1                 # Windows 构建脚本
└── .github/workflows/        # CI/CD
```

### 添加新源

1. 在 `src/rust/` 下创建新目录，例如 `src/rust/zh.example/`
2. 复制 `zh.rouman5` 的结构作为模板
3. 修改 `res/source.json` 和 `src/lib.rs`
4. 运行 `./build.ps1` 构建所有源

### 构建

```bash
# 构建所有源
./build.ps1

# 构建单个源
./build.ps1 -SourceName "zh.rouman5"

# 启动本地服务器
aidoku serve src/rust/zh.rouman5/package.aix
```

## License

MIT
