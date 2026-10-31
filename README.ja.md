# ShotCraft Engine

[English](README.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md) · [Français](README.fr.md)

## 1. 機能概要

### 出典と変更内容

**ShotCraft Engine は [mmartinez/shotcraft](https://github.com/mmartinez/shotcraft) のソースコードを改変・削減した派生プロジェクトです。** 上流の撮影・結合コードと ShotCraft 貢献者の著作権表示を保持しています。独立して保守されており、上流の公式リリースではありません。

元のアプリケーションからの主な変更は次のとおりです。

- 撮影コンポーネントを単一の Rust ライブラリと CLI に抽出し、egui UI、エディター、OCR、トレイ、履歴、アプリ設定、アップロード、MCP、アセットを現在のビルドから除外しました。
- テクスチャ付きパッチと広範囲のピクセル検証によるスクロールフレーム照合に変更し、固定ヘッダー・フッターを保持、重複部分を更新し、最後に採用したフレームを結合基準にします。[LiteSnap](https://github.com/HuibingLin/LiteSnap) のアルゴリズムの考え方を参考にしましたが、製品コード、バイナリ、アセットは含めていません。[ATTRIBUTION.md](ATTRIBUTION.md) を参照してください。
- ホストウィンドウの物理外枠と倍率から画面を選択し、混在 DPI の座標を処理します。Windows CLI は DPI awareness を初期化します。保持している Linux X11 pixmap 撮影経路は上流由来です。残る制限は後述します。
- JPEG/PNG の Base64 API、Rust 側の JPEG 品質指定、順序付き画像リストを提供し、撮影パネルやクリップボード操作なしで組み込めるようにしました。
- Rust の原子フラグと CLI 標準入力の `stop` による協調停止を追加し、取得済み内容を保持します。
- スクロール結合に加え、単一画面 `--single` と PageDown による個別撮影 `--pages` を追加しました。
- 元アプリのパッケージ作成を、macOS ARM64/Intel、Windows x64、Ubuntu x64 向けの手動コンポーネントビルドに変更し、4 言語の組み込み文書を提供します。

### 撮影モード

Rust アプリケーションに組み込める画面全体のキャプチャ用コンポーネントです。単独で使える CLI `scroll_capture` も提供します。

| モード | 送信する入力 | 結果 |
| --- | --- | --- |
| デフォルト / `CaptureMode::Stitched` | 対象ディスプレイの中央へポインターを移動し、下へスクロール | 結合した縦長画像 1 枚 |
| `--single` / `CaptureMode::Single` | なし | 現在の画面全体を 1 枚 |
| `--pages` / `CaptureMode::Pages` | PageDown | 撮影順の画面全体の画像リスト |

呼び出し元のウィンドウ外枠からディスプレイを選択します。JPEG/PNG と Base64 に対応し、リサイズしません。ライブラリはウィンドウ作成、ファイル保存、クリップボード操作を行わず、CLI が保存を担当します。途中停止して撮影済みの内容を保持できます。単一画面モードは実デスクトップで確認済みですが、全プラットフォームの検証完了を意味しません。

## 2. 詳しい使い方

### 構文と引数

角括弧は省略可能な項目を示します。括弧自体は入力しません。

```text
scroll_capture [--single | --pages] <new-output-directory> [x y width height scale_factor]
scroll_capture --help
scroll_capture -h
```

| 引数 | 指定できる値 | デフォルト / 条件 |
| --- | --- | --- |
| `--single` | 出力ディレクトリより前のフラグ。`--pages` と併用不可 | モード指定なしではスクロール結合 |
| `--pages` | 出力ディレクトリより前のフラグ。`--single` と併用不可 | モード指定なしではスクロール結合 |
| `new-output-directory` | 未作成のディレクトリ。親は存在する必要がある。空白を含むパスは引用符で囲む | 必須、上書き不可 |
| `x`, `y` | `i32` 整数、−2,147,483,648～2,147,483,647。ウィンドウ外枠左上の物理座標 | `0`, `0` |
| `width`, `height` | `u32` 整数、1～4,294,967,295。ウィンドウ外枠の物理サイズ | `100`, `100` |
| `scale_factor` | 有限かつ正の `f64`。呼び出し元ウィンドウの倍率。macOS の座標変換に使用 | `1.0` |
| `--help`, `-h` | 単独、または `--single` / `--pages` の直後 | ヘルプのみ表示。撮影・ディレクトリ作成なし |

幾何情報 5 項目は順番どおりすべて指定するか、すべて省略してください。切り抜き範囲ではなくディスプレイ選択用です。重なり面積が最大のディスプレイを選び、重なりがなければ中心が最も近いものを選びます。省略時は `(0, 0, 100, 100, 1.0)` を使い、通常はプライマリ画面に相当するデスクトップ原点付近を選びます。CLI は端末や Pluely14 の位置を自動取得しません。`-` で始まるディレクトリ名は `./--name` のように指定します。

CLI は **JPEG 品質 80**、**準備時間 10 秒**で固定です。`--format`、`--quality`、`--delay` はありません。エンコードの指定には Rust API を使います。

### 実行例

ビルド後、macOS / Ubuntu：

```sh
./target/release/scroll_capture ./capture-01
./target/release/scroll_capture --single ./single-01
./target/release/scroll_capture --pages ./pages-01
```

Windows PowerShell：

```powershell
.\target\release\scroll_capture.exe .\capture-01
.\target\release\scroll_capture.exe --single .\single-01
.\target\release\scroll_capture.exe --pages .\pages-01
```

Cargo では `--` の後にプログラム引数を置きます。2 番目の例は Retina 画面上の物理ウィンドウ外枠を指定しています。

```sh
cargo run --release --locked --bin scroll_capture -- --single ./single-01
./target/release/scroll_capture --pages ./pages-retina-01 200 200 800 600 2
```

各コマンドでは異なる新規ディレクトリを指定してください。準備時間中に対象画面を表示し、スクロール撮影は希望するページ位置から開始します。単一画面モードは現在の表示を撮り、ページ先頭へは戻しません。

ページモードは最初の 1 枚を撮影し、その後は PageDown を 1 回送信して 350 ms 待ってから撮影します。対象ディスプレイ上のページ本文に自分でフォーカスを合わせてください。ポインター移動、クリック、フォーカス取得は行いません。移動量はアプリの PageDown 動作に依存し、ディスプレイの高さと厳密には一致しません。画像間の重複は許容されます。アドレスバー、エディター、入れ子のスクロール領域がキーを受け取る場合があります。

### 出力と停止

| モード | 出力ファイル |
| --- | --- |
| 結合 / 単一画面 | `capture.jpg`, `capture.base64` |
| ページ | `page-0001.jpg`, `page-0001.base64`, `page-0002.jpg`, `page-0002.base64`, … |

`.base64` は対応する JPEG の標準 Base64 で、data URI プレフィックスは含みません。画面全体の画像は撮影時のサイズ、結合画像は結合結果のサイズを保持し、リサイズしません。

標準入力に小文字の `stop` だけを入力して Enter を押すと、以降のスクロールを止め、取得済みの内容を返します。前後に空白を付けないでください。最初のフレームより前の停止はエラーとなり、画像は生成しません。単一画面モードで既に撮影したフレームは保持します。端末への切り替えが画像に混入することがあるため、ホストはパイプ経由で `stop
` を送信できます。

保存はキャプチャ関数が戻った後にまとめて行い、フレームごとには保存しません。`Ctrl+C` や強制終了は未保存の結果を失う可能性があります。通常の撮影エラーでは以前のフレームも保存されず、書き込みエラーでは一部ファイルだけ残る場合があります。成功時の終了コードは 0、失敗時は非 0 です。標準出力はパス、サイズ、JPEG バイト数を表示し、Base64 本文は出力しません。ディレクトリの存在だけで成功とは判断できません。

## 3. 他のソフトウェアから呼び出す

### Rust 依存関係と API

ローカルソースを使う場合はパスを調整します。

```toml
[dependencies]
shotcraft-engine = { path = "../shotcraft-engine" }
```

Git 依存関係も使えます。非公開リポジトリではビルド環境に Git アクセス権が必要です。

```toml
[dependencies]
shotcraft-engine = { git = "https://github.com/teddyxiong/shotcraft-engine.git", branch = "main" }
```

Cargo がライブラリをホストに組み込みます。C ABI DLL や HTTP サービスをそのまま提供するものではありません。公開 API は `shotcraft_engine::scrolling` にあります。

```rust,ignore
pub fn capture_to_base64(
    bounds: WindowBounds,
    format: Option<String>,
    quality: Option<u8>,
    stop: &std::sync::atomic::AtomicBool,
    mode: CaptureMode,
) -> shotcraft_engine::Result<Vec<String>>;

pub fn encode_image_to_base64(
    image: &image::RgbaImage,
    format: Option<String>,
    quality: Option<u8>,
) -> shotcraft_engine::Result<String>;
```

| API 引数 | 値と動作 |
| --- | --- |
| `bounds: WindowBounds` | 必須フィールド：`x: i32`, `y: i32`, `width: u32`, `height: u32`, `scale_factor: f64`。単位、範囲、選択規則は CLI と同じ。ライブラリ側のデフォルトはなし |
| `format: Option<String>` | 小文字の `Some("jpg".into())` または `Some("jpeg".into())` のみ JPEG。`None`、`"png"`、その他は PNG。大文字小文字を区別 |
| `quality: Option<u8>` | 入力は 0～255。`None` は 80。JPEG は 1～100 に制限し、PNG では無視。JPEG の透明部分は白背景に合成 |
| `stop: &AtomicBool` | 必須。`false` で初期化し、別スレッドやイベントハンドラーから `true` にして停止 |
| `mode: CaptureMode` | 必須：`Single`、`Stitched`、`Pages`。デフォルトなし |
| `image: &image::RgbaImage` | エンコーダー専用。既存の RGBA 画像を渡し、撮影や入力は行わない |

ワーカースレッドで単一画面を撮影する例です。外枠はホストの物理位置、外枠サイズ、ウィンドウ自身の倍率に置き換えます。Tauri では `outer_position`、`outer_size`、`scale_factor` などを使います。

```rust
use shotcraft_engine::scrolling::{CaptureMode, WindowBounds, capture_to_base64};
use std::sync::{Arc, atomic::AtomicBool};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bounds = WindowBounds {
        x: 200, y: 200, width: 800, height: 600, scale_factor: 2.0,
    };
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = Arc::clone(&stop);
    let task = std::thread::spawn(move || {
        capture_to_base64(
            bounds, Some("jpeg".into()), Some(80), &worker_stop,
            CaptureMode::Single,
        )
    });
    let images = task.join()
        .map_err(|_| std::io::Error::other("capture worker panicked"))??;
    println!("Captured images: {}", images.len());
    Ok(())
}
```

縦長画像には `CaptureMode::Stitched`、個別ページには `CaptureMode::Pages` を指定します。`Single` と `Stitched` は長さ 1 のリスト、`Pages` は採用した画像ごとの文字列を返します。成功時は少なくとも 1 枚あり、関数終了時に一括で返します。フレームごとのコールバックはありません。

API は同期・ブロッキングで、CLI の 10 秒待機はありません。ワーカーまたはホストの `spawn_blocking` で実行し、UI イベントスレッドで同期的に `join` しないでください。表示制御、フォーカス、権限、ショートカット登録はホストの責任です。Tauri 依存関係や組み込みのグローバル停止キーはありません。

`Arc<AtomicBool>` をアプリ状態に保持し、停止コマンドやショートカットの処理で実行します。

```rust,ignore
stop.store(true, std::sync::atomic::Ordering::Relaxed);
```

タスクごとに新しいフラグを作り、実行中にリセットしないでください。停止時は確定済み画像を保持します。進行中の撮影・結合・エンコード・残りの 350 ms 待機は強制中断できず、送信済み入力も取り消せません。単一画面ではこの待機、すべての入力、連続撮影バックエンド、結合処理を省略します。

既存画像を直接エンコードする場合：

```rust,ignore
let base64 = shotcraft_engine::scrolling::encode_image_to_base64(
    &image, Some("jpeg".into()), Some(80),
)?;
```

Base64 をデコードすると指定形式の画像バイト列になります。受信側が素の Base64 を受け取るならそのまま渡し、必要なら `data:image/jpeg;base64,` / `data:image/png;base64,` を付けます。ページは順番に個別画像として渡してください。Base64 文字列の連結は 1 枚の画像にはなりません。AI サービスや Pluely14 への自動送信は行いません。

### CLI を子プロセスとして使う

対象 OS・アーキテクチャ用の実行ファイルと必要なライセンスを同梱し、前述の引数と新規出力ディレクトリで起動します。標準入力をパイプで保持し、停止時は `stop
` を書いてフラッシュします。標準入力を閉じるだけでは停止しません。終了コード 0 を確認してから `.jpg` / `.base64` を読みます。CLI は単独実行ファイルで、Rust への直接組み込みは前述のソース依存関係を使います。

## 4. プラットフォームと制限

### 現在の動作と上限

- 結合：最大 50 回の撮影試行、または高さ 40,000 ピクセル。3 回連続で変化がなければ終了し、3 回連続で一致を検出できなければエラーです。上限では取得済み部分を返し、ページ末尾までの到達は保証しません。1 ステップは縦ホイール 3 単位と 350 ms 待機です。
- ページ：最大 50 回の撮影試行。連続する完全に同一の RGBA フレームを省略し、3 回連続で不変なら終了します。類似度による除外、結合可能性の条件、高さ 40,000 ピクセルの結合上限はありません。動画・アニメーション・システム表示の変化で末尾判定できない場合は、停止要求か回数上限を使います。
- 単一画面：入力や結合なしで 1 回撮影します。画面取得権限は必要で、これだけでネイティブ Wayland 対応を意味しません。
- 後続の撮影・入力エラーと停止が同時に発生した場合は、確定済み内容を優先します。専用ウィンドウがなくても他の可視内容は写り、スクロールモードは実際のデスクトップを変化させます。
- macOS：呼び出しプロセスに画面収録権限が必要です。スクロールモードでは自動入力に必要な権限も必要です。
- Windows：単独 CLI は DPI awareness を初期化しますが、ライブラリはホストのプロセス DPI 設定を変更しません。結合時の Enigo ポインター位置はプライマリ画面基準で正規化されるため、セカンダリ画面のホイール操作は既知の制限があります。まずプライマリで確認してください。ページモードはフォーカス先にキーを送ります。セカンダリは実機検証が必要です。
- Ubuntu：現在のスクロール入力は X11 です。`echo "$XDG_SESSION_TYPE"` で確認できます。ネイティブ Wayland のスクロールは未対応です。Ubuntu 24.04 でビルドし、22.04 コンテナーの `--help` は起動だけを検証します。

Ubuntu の実行時ライブラリ：

```sh
sudo apt-get install -y libpipewire-0.3-0 libxcb1 libgbm1 libegl1 libx11-6 libwayland-client0
```

### ネイティブ Wayland：適応条件と現状

固定された xcap には Wayland の単発撮影経路がありますが、持続的な Portal/PipeWire セッションは実装していません。Enigo はデフォルトの X11 入力バックエンドです。開発パッケージの導入、ビルド成功、単発撮影成功だけでは Wayland の自動スクロールは証明できません。

| 必要な機能 | 用途 |
| --- | --- |
| 実際の Wayland デスクトップとユーザーセッション D-Bus | デスクトップサービスとの通信と実動作検証 |
| xdg-desktop-portal と対応するデスクトップバックエンド | GNOME/KDE などで必要なインターフェースを提供 |
| ScreenCast Portal と PipeWire | 許可されたセッションで対象画面のフレームを連続取得 |
| RemoteDesktop Portal の入力機能 | 結合にはポインター/ホイール、ページには PageDown。対応メソッドまたは libei/EIS を使用 |
| 画面と対象入力デバイスの許可 | 画面の許可だけではポインターやキーボード入力は許可されない |

対応方針は、許可済み撮影・入力セッションを再利用し、ストリームの位置・サイズ・倍率を対象画面に対応付け、既存の結合・エンコード・停止へ接続することです。Enigo の feature を 1 つ有効にするだけでは不十分です。コンポジター座標を物理ピクセルと仮定せず、外枠の重なりによる選択も検証する必要があります。

専用パネルを作らなくても、通常の Portal ではシステムの許可・画面選択 UI が表示されます。対応環境では `persist_mode` と `restore_token` で再確認を減らせますが、トークン更新が必要で、権限取り消し、画面変更、復元失敗で再表示されます。初回から無表示、または永久に確認不要とは保証できません。

グローバル停止キーにはホストによる GlobalShortcuts Portal の確認と登録も必要です。まず Ubuntu 24.04 + GNOME Wayland で連続取得、入力、結合、途中停止を検証し、次に KDE などを確認します。Ubuntu のバージョンだけでは判断できません。単一画面も既存バックエンドを使うため、取得可否と許可 UI はデスクトップに依存します。

Portal 資料：[ScreenCast](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.ScreenCast.html) · [RemoteDesktop](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html) · [GlobalShortcuts](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)

## 5. ローカルと GitHub でのビルド

### 準備とローカルビルド

**Rust 1.95 以降**と Cargo を用意し、ソース取得後にツールチェーンを確認します。

```sh
git clone https://github.com/teddyxiong/shotcraft-engine.git
cd shotcraft-engine
rustc --version
cargo --version
```

rustup を使っている場合は `rustup update stable` で安定版を更新できます。非公開リポジトリへのアクセスは事前に設定してください。

| プラットフォーム | ビルド要件 |
| --- | --- |
| macOS | Xcode Command Line Tools（`xcode-select --install`）とネイティブ Rust ツールチェーン |
| Windows x64 | Visual Studio C++ Build Tools、Windows SDK、`x86_64-pc-windows-msvc` Rust ツールチェーン。設定済みの開発者端末を使用 |
| Ubuntu 24.04 x64 | Rust と以下の開発パッケージ |

```sh
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libclang-dev libxcb1-dev libgbm-dev libegl-dev libpipewire-0.3-dev libspa-0.2-dev libwayland-dev
```

リポジトリルートで CLI をビルド、またはライブラリをチェックします。

```sh
cargo build --release --locked --bin scroll_capture
cargo check --lib --locked
```

出力は `target/release/scroll_capture`（macOS/Ubuntu）または `target/release/scroll_capture.exe`（Windows）です。他の Rust プロジェクトが依存すると Cargo がライブラリを自動ビルドします。`--target <triple>` 指定時は `target/<triple>/release/` に出力されます。

プロジェクトのチェック：

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

### GitHub Actions の手動ビルド

有効なワークフローは [engine-build.yml](.github/workflows/engine-build.yml) の **Engine native builds** のみです。トリガーは `workflow_dispatch` のみで、push、PR、タグ、定期実行では開始しません。

1. [Actions](https://github.com/teddyxiong/shotcraft-engine/actions/workflows/engine-build.yml) を開きます。
2. **Run workflow** で対象変更を含むブランチを選び、再度 **Run workflow** を押します。
3. 4 ジョブの完了後、実行ページから対象 Artifact を取得します。保存期間は 14 日です。

| プラットフォーム | ターゲット triple | Artifact |
| --- | --- | --- |
| macOS ARM64 | `aarch64-apple-darwin` | `scroll-capture-macos-arm64` |
| macOS Intel | `x86_64-apple-darwin` | `scroll-capture-macos-x64` |
| Windows x64 | `x86_64-pc-windows-msvc` | `scroll-capture-windows-x64` |
| Ubuntu x64 | `x86_64-unknown-linux-gnu` | `scroll-capture-ubuntu-x64` |

ジョブはフォーマット、Clippy、テスト、release ビルド、`--help` を実行し、Ubuntu では 22.04 の起動チェックも行います。Artifact には実行ファイル、英語 README、ライセンス、出典、コミット、ツールチェーン情報が含まれます。他言語版はリポジトリで参照できます。

展開先で `./scroll_capture --single ./single-01`、Windows では `.\scroll_capture.exe --single .\single-01` を実行します。macOS/Ubuntu では必要に応じて先に `chmod +x ./scroll_capture` を実行します。スクロール検証は第 2 節の引数と毎回新しいディレクトリを使い、実デスクトップで全体取得と停止を確認してください。CI のビルド・ヘルプ確認だけでは実動作を保証しません。

## 6. ソースとライセンス

### ライセンスと利用条件

この改訂に含まれる、保守者が許諾権を持つ独自の貢献・変更は、独自の [ShotCraft Engine Source-Available License 1.0](LICENSE) に従います。現在の改訂全体を一括して MIT/Apache で提供するものではありません。正式な条件は英語の `LICENSE` で、本節は説明用の要約です。

| 利用形態 | 許諾 |
| --- | --- |
| 個人・企業・組織が公開し、アクセス・利用・機能の料金を利用者から取らないソフトウェア | `LICENSE` に従って無料 |
| 広告、任意のスポンサー支援、任意の寄付で運営する無料ソフトウェア | アクセス・利用・機能に支払いを要求しない場合は無料 |
| 企業・組織自身の内部利用（事業運営を含む） | `LICENSE` に従って無料 |
| 購入、購読、ライセンス料、有料プラン、機能解除、ホスト型ソフトウェアへのアクセスで利用者に課金するソフトウェア | 撮影機能自体が無料でも、別途書面による有償商用ライセンスが必要 |

ライブラリ組み込み、組み込んだ CLI の呼び出し、改変版にも適用されます。この独自ライセンスはホストアプリや非公開変更のソース公開を要求しません。有償許諾は[リポジトリの Issue](https://github.com/teddyxiong/shotcraft-engine/issues) で保守者に連絡し、価格と範囲を別途書面で合意します。問い合わせや寄付だけでは許諾になりません。

上流 ShotCraft 部分は **MIT OR Apache-2.0** と著作権表示を保持し、依存関係はそれぞれのライセンスに従います。独自条項は保守者が許諾できる部分のみを対象とし、上流や過去の独立した許諾を取り消しません。特に **`v0.1.1`（`114184a9a7a37928c818b1aa8b804bcf02c869a7`）および以前の版は元の条件を保持します**。有効な過去の寛容な許諾を持つ人は、その許諾で対象の旧コードを使い続けられ、新条項によって既存利用者に支払いを強制するものではありません。

これは有料ソフトウェア制限付きの **source-available** ライセンスで、OSI 承認のオープンソースライセンスではありません。`v0.1.1` タグは変更していません。パッケージ版 `0.1.1` だけでは main の後続コミットの条件を識別できないため、正確なコミットを固定して確認してください。新しいリリースタグは作成しません。

変更や派生版を公開する際は Issue での通知を歓迎します。任意のお願いであり、追加条件でも必要な有償許諾の代わりでもありません。

| ファイル | 役割 |
| --- | --- |
| `src/lib.rs` | 公開項目と構造化エラー |
| `src/capture.rs` | Linux X11 pixmap 経路を含む画面取得 |
| `src/stitch.rs` | ピクセルベースのスクロール画像結合 |
| `src/scrolling.rs` | 画面選択、モード、停止、Base64 エンコード |
| `src/bin/scroll_capture.rs` | CLI 引数とファイル出力 |

上流の表示と条文は [LICENSE-MIT](LICENSE-MIT)、[LICENSE-APACHE](LICENSE-APACHE)、[NOTICE](NOTICE)、[ATTRIBUTION.md](ATTRIBUTION.md) に保持し、依存関係の条文は生成された [THIRD-PARTY-LICENSES.md](THIRD-PARTY-LICENSES.md) にあります。再配布や製品組み込みでは [LICENSE](LICENSE) と適用される上流・依存関係の表示を同梱してください。元の ShotCraft コードのライセンスを変更するものではありません。

元の UI、OCR、エディター、トレイ、アセットは現在のコンポーネントから除外しています。無視対象のローカル `backups/` は復旧用であり、Git 配布物やビルドには含まれません。
