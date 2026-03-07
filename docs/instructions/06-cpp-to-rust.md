# 指示書 #06: cpp-to-rust 変換エンジン担当

## あなたの役割

あなたは**cpp-to-rust**プロジェクトの**リード開発者**です。
C/C++のコードベースをRustに変換するAIエージェントを構築します。

- リポジトリ: https://github.com/LegacyToRustProject/cpp-to-rust
- 言語: Rust
- LLM: Claude API（デフォルト。差し替え可能に設計）
- ライセンス: MIT

---

## なぜC/C++

- メモリ安全性の脆弱性が全セキュリティバグの~70%（Microsoft, Google, NSA調査）
- White House, NSA, DARPAがメモリ安全言語への移行を推奨
- DARPAのTRACTORプログラムがC→Rust変換に取り組んでいるが、形式検証アプローチで実用化は遠い
- 我々はAI + 出力比較の実践的アプローチで、より早く実用レベルに到達する

---

## アーキテクチャ

```
cpp-to-rust/
├── Cargo.toml
├── crates/
│   ├── cpp-parser/              # C/C++ ソースコード解析
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── analyzer.rs      # ファイル構造・依存関係分析
│   │       ├── preprocessor.rs  # #include, #define の解決
│   │       ├── memory.rs        # メモリパターン分析（malloc/free, new/delete）
│   │       └── types.rs         # C/C++構造体定義
│   ├── rust-generator/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── llm.rs           # LLMプロバイダー
│   │       ├── prompt.rs        # C/C++専用変換プロンプト
│   │       ├── ownership.rs     # 所有権推論ヒント生成
│   │       └── generator.rs
│   ├── verifier/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── compiler.rs      # cargo check
│   │       ├── comparator.rs    # C/C++ vs Rust 出力比較
│   │       ├── sanitizer.rs     # AddressSanitizer等での検証
│   │       └── fix_loop.rs
│   └── cli/
│       └── src/
│           └── main.rs
├── profiles/
│   ├── c99/
│   ├── c11/
│   ├── cpp11/
│   ├── cpp17/
│   └── embedded/               # 組み込みC向け
└── tests/
    └── fixtures/
```

---

## C/C++固有の課題と変換戦略

### ポインタ → 所有権

C/C++の最大の変換課題。AIに所有権パターンを推論させる。

```c
// 所有権: 呼び出し元が所有
char* create_string(const char* input) {
    char* result = malloc(strlen(input) + 1);
    strcpy(result, input);
    return result;  // caller must free
}
```

**変換**:
```rust
fn create_string(input: &str) -> String {
    input.to_string()  // 所有権は戻り値で移動
}
```

### プリプロセッサマクロ

```c
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define BUFFER_SIZE 1024
#ifdef DEBUG
  #define LOG(msg) printf("[DEBUG] %s\n", msg)
#else
  #define LOG(msg)
#endif
```

**変換戦略**:
```rust
// 関数マクロ → Rustマクロ or ジェネリック関数
fn max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b { a } else { b }
}

// 定数マクロ → const
const BUFFER_SIZE: usize = 1024;

// 条件コンパイル → cfg
#[cfg(debug_assertions)]
fn log(msg: &str) { eprintln!("[DEBUG] {}", msg); }
#[cfg(not(debug_assertions))]
fn log(_msg: &str) {}
```

### malloc/free → RAII

```c
void process() {
    int* data = (int*)malloc(100 * sizeof(int));
    if (!data) return;
    // ... use data ...
    free(data);
}
```

**変換**:
```rust
fn process() {
    let mut data = vec![0i32; 100];
    // ... use data ...
    // 自動解放（RAII）
}
```

### C++テンプレート → Rustジェネリクス

```cpp
template<typename T>
class Stack {
    std::vector<T> data;
public:
    void push(T value) { data.push_back(value); }
    T pop() { T val = data.back(); data.pop_back(); return val; }
};
```

**変換**:
```rust
struct Stack<T> {
    data: Vec<T>,
}

impl<T> Stack<T> {
    fn push(&mut self, value: T) { self.data.push(value); }
    fn pop(&mut self) -> Option<T> { self.data.pop() }
}
```

### undefined behavior の排除

Rustに変換するだけで以下のUBが構造的に排除される:
- NULLポインタ参照 → Option<T>
- バッファオーバーフロー → 境界チェック
- use-after-free → 所有権システム
- データ競合 → Send/Sync

---

## バージョン互換

### C Track
| Standard | Priority | Notes |
|----------|----------|-------|
| C99 | **First** | Linux kernel baseline |
| C11 | **First** | Threads, atomics |
| C89 | Third | 古いコードベース。シンプル |
| C17/C23 | Fourth | 最新。採用少 |

### C++ Track
| Standard | Priority | Notes |
|----------|----------|-------|
| C++17 | **First** | モダン機能、広い採用 |
| C++11/14 | **First** | Lambda, move semantics |
| C++03 | Third | 企業レガシー |
| C++20/23 | Fourth | 最新 |

---

## テストフィクスチャ

```
tests/fixtures/
├── c/
│   ├── 01_hello_world/
│   ├── 02_pointer_arithmetic/
│   ├── 03_struct_and_malloc/
│   ├── 04_file_io/
│   ├── 05_linked_list/
│   └── 10_small_project/
└── cpp/
    ├── 01_class_basic/
    ├── 02_inheritance/
    ├── 03_template/
    ├── 04_stl_containers/
    ├── 05_smart_pointers/
    └── 10_small_project/
```

---

## 完了条件

- [ ] `cpp-to-rust analyze` でC/C++プロジェクトの構造レポート出力
- [ ] `cpp-to-rust convert` でC/C++プロジェクトがRustに変換される
- [ ] ポインタ→所有権の変換が動作する
- [ ] malloc/free→Vec/Box/RAII変換が動作する
- [ ] プリプロセッサマクロが適切に変換される
- [ ] cargo check + 出力比較の検証ループが動作する
- [ ] C99とC++17のテストフィクスチャで変換成功
- [ ] 全テスト通過、clippy通過

---

## 自律的に動くこと

**あなたは自分で判断して進めてください。**

## 判断原則

1. **安全性向上が第一目標。** C/C++→Rustの最大の価値はメモリ安全性。unsafe Rustを極力使わない変換を目指す。
2. **DARPAと差別化する。** 形式検証ではなく実践的な出力比較で品質を保証する。
3. **Cを先にやる。** C++テンプレートは複雑。まずC99で動くものを作り、C++に拡張する。
4. **php-to-rustと設計を揃える。** LLMプロバイダー、検証ループの設計パターンは共通化する。


---

## ビジネスモデル（次フェーズ）

この変換エンジンは2つの形態で提供される。現時点ではCLI開発に集中し、ホスト型は次フェーズで実装する。

| | セルフホスト (CLI) | ホスト型 (Web UI) |
|---|---|---|
| 料金 | 無料（OSS） | Stripe課金 |
| APIキー | ユーザーが自分で用意 | サービス側が提供 |
| 対象 | 開発者 | 非開発者 |

**現時点での影響:**
- APIキーはハードコードしない（環境変数 or 設定ファイル）
- APIキーをログに出力しない
- LlmProviderトレイトの差し替え可能な設計を維持する（将来のマルチプロバイダー対応）

---

## QAレビューの確認

あなたの成果物はQA担当（#09）によってレビューされる。レビュー結果は以下に置かれる：

```
/home/ubuntu/RustPress-1/reviews/
```

**あなたの責任:**
1. レビューファイルを定期的に確認せよ
2. ブロッカー（blocker）指摘があれば即座に修正せよ
3. 警告（warning）指摘はマージ前に対応せよ
4. 提案（suggestion）は自分の判断で対応可否を決めてよい
5. 修正したらQA担当に再レビューを依頼せよ
