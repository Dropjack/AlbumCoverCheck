# Album Cover Check

Album Cover Check 是一个用于检查本地音乐库封面的工具。它的作用很简单：确认音频文件里是否真的嵌入了专辑封面。

很多桌面播放器会让你误以为“这张专辑有封面”。实际上，播放器只是读取了同一目录里的 `cover.jpg`、`folder.jpg` 等图片文件，而不是音频文件内部真正嵌入的封面。

这样的问题在把音乐拷到手机、播放器或其他设备时就会暴露出来——如果设备只识别嵌入式封面，原本看起来正常的专辑就会突然变成没有封面。

Album Cover Check 的目的，就是把这些“依赖外部图片、但音频文件里没有封面”的专辑或歌曲找出来。

## 特性

* 检查 `mp3`、`m4a`、`mp4`、`flac` 文件中是否包含嵌入式 `Front Cover`
* 统计但暂时跳过未支持的音频格式：`aac`、`aiff`、`alac`、`ape`、`dsd`、`dsf`、`ogg`、`opus`、`wav`、`wma`
* 自动忽略一些常见的系统文件，例如：`._*`、`Thumbs.db`、`desktop.ini`、`.DS_Store`
* 支持 `text`、`csv`、`json` 三种输出格式
* 支持按修改时间筛选文件：`modified_within_days` / `--modified-within-days`
* 提供终端界面（TUI），同时保留 `--plain` 纯文本模式
* 如果检测到外部 `cover.jpg` / `folder.jpg` / `front.png`，会给出提示，但不会把它当作“已经有封面”

## 当前功能范围

目前工具的检查范围比较明确：

* 只检查音频文件中的嵌入式 `Front Cover`
* 不检查 `Back Cover`、`Artist`、`Icon`、`Disc` 等其他类型图片
* 不把外部 `jpg/png` 文件当作“已有封面”
* 不根据目录名推测专辑结构，而是直接递归扫描整个目录
* 对外部图片文件名只识别常见名称，不尝试处理各种乱码或非常规命名

## 快速开始

1. 打开根目录下的 `album_cover_check.toml`
2. 修改扫描目录、输出目录和输出格式
3. 运行 `album_cover_check.exe`

程序会优先读取当前目录中的 `album_cover_check.toml`。如果命令行里同时传入参数，则以命令行参数为准。

## 配置文件

常用配置项只有四个：

* `scan_root`
* `output_dir`
* `output_format`
* `modified_within_days`

Windows 路径建议使用单引号，例如：

```toml
scan_root = 'C:\Users\YourName\Music'
output_dir = 'D:\'
```

如果使用双引号，需要把反斜杠写成 `\\`。

程序会根据 `output_format` 自动生成固定文件名：

* 主报告：`cover_checklist.txt` / `cover_checklist.csv` / `cover_checklist.json`
* 错误日志：`cover_check_errors.txt` / `cover_check_errors.csv` / `cover_check_errors.json`

## 命令行用法

```text
album_cover_check [SCAN_ROOT] [OPTIONS]
```

常用参数：

* `--output-dir <PATH>`：输出目录
* `--format <text|csv|json>`：输出格式
* `--modified-within-days <DAYS>`：只扫描最近 N 天修改过的文件
* `--config <PATH>`：指定配置文件路径
* `--plain`：使用纯文本模式（不启用终端界面）
* `--help`：显示帮助信息

示例：

```powershell
album_cover_check E:\Music --format json
album_cover_check --config .\album_cover_check.toml
album_cover_check E:\Music --modified-within-days 30 --output-dir D:\AlbumCoverCheck --format csv
```

## 输出内容

主报告包含以下信息：

* 扫描的根目录
* 输出格式
* 修改时间筛选条件
* 扫描到的支持格式文件数量
* 各音频格式的数量分布
* 缺少嵌入式 `Front Cover` 的歌曲数和专辑数
* 被跳过的不支持格式文件数量及其分布
* 缺少封面的专辑列表
* 每张专辑是否检测到常见的外部封面图片

错误日志包含：

* 无法读取元数据的音频文件
* 对应的专辑名
* 错误信息

## 仓库说明

* `README.md`：项目使用说明
* `sample.config.toml`：示例配置文件（中文）

## 后续计划

* 目前只支持 Windows，未来可能考虑扩展到 macOS / Linux
* 改进 `m4a/mp4` 在某些标签编码情况下的兼容性
* 评估是否正式支持 `APE`
* 评估是否正式支持 `OGG/Opus`
* 继续优化终端界面（TUI）和发布包结构
