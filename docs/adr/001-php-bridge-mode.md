# ADR-001: PHP Bridge Mode の採否とプラグイン互換性戦略

- **ステータス**: 承認
- **日付**: 2026-03-07
- **関連**: [GitHub Discussion #1](https://github.com/rustpress-project/RustPress/discussions/1)

---

## 結論（先に読む人向け）

**PHP Bridge Modeは採用しない。PHPプラグインをPHPのまま動かすのではなく、AIでRustに変換する。**

理由: WordPressのソースコードは100%オープンソースであり、「正解」が完全に読める。正解が存在する以上、AIによる変換と正解との比較検証を繰り返せば、100%の互換性は「不可能」ではなく「作業量」の問題である。

```
WordPress (PHP) = 正解のソースコード（仕様書そのもの）
          ↓ AIが読んで変換
RustPress (Rust) = 正解と同じ動作をするRust実装
          ↓ 正解と比較テスト
差分があれば修正（正解が存在するから必ず直せる）
```

---

## コンテキスト

RustPressの最終目標は「全世界の全WordPressサイトの移行経路を確立する」ことである (MASTERPLAN Phase 11完了基準)。

WordPressエコシステムには59,000以上のPHPプラグインが存在し、多くのサイトがこれらに依存している。このプラグイン互換性をどう実現するかは、プロジェクトの成否を左右する最重要のアーキテクチャ決定である。

検討された2つのアプローチ:
1. **PHP Bridge Mode**: PHPプラグインをPHPのまま動かす（RustPressからPHP-FPMを呼び出す）
2. **AI全変換**: PHPコードをAIでRustに変換する（PHPランタイム不要）

---

## PHP Bridge Mode — 検討と却下

### なぜ検討したか

59,000のPHPプラグインをRustに変換するのは膨大な作業に見えた。PHPプラグインがそのまま動けば移行障壁はゼロになる。

### 案A: フックごとにPHP-FPM呼び出し — 却下

```
RustPress → [フック発火100回 × FastCGI往復] → Response
```

1ページあたり100-300回のフック発火があり、各フック発火でFastCGI往復 (~0.5-1ms) が発生。追加オーバーヘッド50-300msで、**WordPressの200msより遅くなる**。

### 案B: リクエストごとに1回PHP-FPM呼び出し — 却下

```
RustPress → [1回のFastCGI: WordPress bootstrap + 全フック実行] → Response
```

動的ページでWordPressの約1.3倍速。ただしこの構成の正体は「WordPressの前にRustのリバースプロキシを置いた」だけであり、RustPressの存在意義が「高速プロキシ」に矮小化される。

さらに致命的な問題: ユーザーがPHP Bridge Modeで「動いてるからいいや」となり、Rustプラグインへの移行が進まない。歴史的前例としてWine（WindowsアプリをLinuxで実行）は20年経ってもLinuxネイティブアプリの普及に貢献しなかった。

### 案C: WordPress bootstrapなしでPHPプラグインフックだけ実行 — 却下

PHPプラグインのコードはWordPress全体の実行コンテキストに依存している:
- `get_option()`, `get_post_meta()` 等のWP関数を呼ぶ
- `$post`, `$wp_query` 等のグローバル変数を参照する
- プラグイン自身のクラスは `require_once` で読み込まれて初めて定義される

WordPress bootstrapなしでは動作しない → 案Bと同じになる。

WP関数をRust側にブリッジする案も検討したが、PHPコールバック内でWP関数を呼ぶたびにIPC往復が発生する「ピンポン地獄」となり、WordPressの5-7倍遅くなる。

### 案D: Hook Connector（RustPressが主、WordPressをHookバックエンドとして使用） — 却下

```
Client → RustPress (主役) → Hook Connector → WordPress (プラグイン実行エンジン)
```

Hook単位のConnectorは案A/Cと同じ問題に帰着。ルート（URL）単位のConnectorは実用的だが、実質リバースプロキシであり案Bと本質的に同じ。

### PHP Bridge全案の根本的問題

どの方式でも、PHPプラグインを動かすにはWordPressランタイムが必要。WordPressを動かすなら、RustPressの意味は「高速キャッシュ/プロキシ」に限定される。これはNginx + Varnishで実現できることであり、Rustで新しいCMSを作る意義がない。

---

## 転換点: 「正解のソースコードが存在する」

PHP Bridge の議論で「PHPプラグインはWordPressランタイムに依存するから動かない」と結論づけかけた。しかし、ここで根本的な問い直しが起きた:

**「WordPressという正解のソースコードが存在するのに、完璧に再現できないのはおかしい」**

WordPressは100%オープンソースである:
- WordPress Core: ~400,000行のPHP — 全行読める
- WP関数 2,000個: 全てオープンソース — コードが仕様書
- 59,000プラグイン: 全てオープンソース — 全コード読める

「PHPプラグインがWordPressに依存する」のは事実だが、**そのWordPress自体がオープンソースのPHPコード**である。つまり:

```
✗ 誤った認識: 「PHPプラグインをRustで動かすのは技術的に困難」
✓ 正しい認識: 「WordPressのPHPコードをRustに変換する作業量が多い」
```

「困難」ではなく「作業量」。そしてAIがある。

---

## 決定: AI全変換戦略

### 方針

```
Step 1: WordPress Core (wp-includes/) をRustに変換
  get_option()       → Rust実装 ← PHPソースコードが仕様書
  WP_Query           → Rust実装 ← PHPソースコードが仕様書
  wp_insert_post()   → Rust実装 ← PHPソースコードが仕様書
  ... 2,000関数全て   → Rust実装 ← PHPソースコードが仕様書

Step 2: グローバル状態もRustで再現
  $post     → AppState内のcurrent_post
  $wp_query → AppState内のmain_query
  $wpdb     → SeaORM DatabaseConnection

Step 3: PHPプラグイン/テーマをAIでRustに変換
  変換後のコードはStep 1のRust版WP関数を呼ぶ
  PHPとRustの関数が1:1対応 → AI変換の精度が最大化

Step 4: 正解との比較テスト
  同じDB + 同じリクエスト → WordPress(PHP)の出力とRustPress(Rust)の出力を比較
  差分があれば修正 → 正解が常に存在するから必ず修正可能
```

### なぜこれが「完璧な回答」と言えるか

| 条件 | 状況 |
|------|------|
| 仕様は明確か？ | Yes — WordPressのPHPソースコードが仕様そのもの |
| AIで変換可能か？ | Yes — PHPもRustもLLMが理解する |
| 検証可能か？ | Yes — WordPress出力と比較すれば差分が検出できる |
| 修正可能か？ | Yes — 差分の原因はPHPコードを読めば特定できる |
| 100%到達可能か？ | Yes — 上記のループを回せば理論的に100%に収束する |

### WP関数互換レイヤーの方針

AI変換のターゲットは「WP互換API」とする（RustPress独自APIではない）:

```
PHPプラグイン:  get_option('key')      →  AI変換  →  Rustプラグイン: get_option("key")
PHPプラグイン:  add_filter('hook', fn) →  AI変換  →  Rustプラグイン: add_filter("hook", fn)
```

PHPとRustのAPI名が1:1対応することで:
- AI変換の精度が最大化される（関数名の置き換えが主な作業になる）
- 変換後のコードが元のPHPコードと対比しやすい
- WP関数の実装仕様はPHPソースコードそのもの（曖昧さゼロ）

---

## WordPress互換性の範囲

| レイヤー | 方針 | 根拠 |
|---------|------|------|
| **DB** (wp_posts等) | **互換維持** | 既存WP DBにそのまま接続 = RustPressの存在意義。これがなければGhost/Strapiとの差別化が消える |
| **REST API** (/wp-json/) | **互換維持** | 既存クライアント/モバイルアプリがそのまま動く |
| **URL構造** | **互換維持** | SEO順位維持。移行の必須条件 |
| **WP関数** (2,000個) | **Rustで再実装** | AI変換されたプラグインが呼び出す先。PHPソースが仕様書 |
| **プラグイン** | **AIでRust変換** | PHPのまま動かすのではなく、Rustに変換する |
| **テーマ** | **AIでTera変換** | PHPテンプレートをTeraテンプレートに変換する |

---

## 影響

### ポジティブ
- PHPランタイムが完全に不要 → 単一バイナリ配布、セキュリティ向上、100倍高速
- 「正解が存在する」ので品質保証が可能 → WordPress出力との比較テストで検証
- AI変換のスケーラビリティ → 59,000プラグインも理論的に変換可能

### リスク
- WP関数2,000個のRust実装は大きな作業量 → ただしAIで加速可能、仕様は明確
- AI変換の精度がプロジェクト成否を左右する → 比較テストで品質を担保
- WordPressのバージョンアップへの追従が必要 → WP関数の差分を検出して更新

### 却下されたもの
- PHP Bridge Mode (案A/B/C/D) — 全て根本的問題あり
- RustPress独自API — WP互換APIのほうがAI変換精度が高い
- DB互換の廃止 — RustPressの存在意義がなくなる
