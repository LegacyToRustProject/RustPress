# 指示書 #07: java-to-rust 変換エンジン担当

## あなたの役割

あなたは**java-to-rust**プロジェクトの**リード開発者**です。
Java EE/SpringのコードベースをRustに変換するAIエージェントを構築します。

- リポジトリ: https://github.com/LegacyToRustProject/java-to-rust
- 言語: Rust
- LLM: Claude API（デフォルト。差し替え可能に設計）
- ライセンス: MIT

---

## なぜJava

- 企業のJava EE/EJBモノリスが数百万行規模で残存
- JVMメモリ: GBクラス → Rust: MBクラス（100倍のコスト削減）
- クラウド移行のボトルネック（コンテナ化するとJVMのオーバーヘッドが顕在化）
- Spring Bootが新規をモダン化したが、レガシーEEアプリは取り残されている

---

## アーキテクチャ

```
java-to-rust/
├── Cargo.toml
├── crates/
│   ├── java-parser/             # Java ソースコード解析
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── analyzer.rs      # パッケージ・クラス構造分析
│   │       ├── dependency.rs    # Maven/Gradle 依存関係解析
│   │       ├── annotations.rs   # アノテーション解析（@Entity, @RestController等）
│   │       └── types.rs
│   ├── rust-generator/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── llm.rs
│   │       ├── prompt.rs        # Java専用変換プロンプト
│   │       ├── patterns.rs      # Javaパターン → Rustパターンのマッピング
│   │       └── generator.rs
│   ├── verifier/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── compiler.rs
│   │       ├── comparator.rs    # Java vs Rust 出力比較
│   │       └── fix_loop.rs
│   └── cli/
│       └── src/
│           └── main.rs
├── profiles/
│   ├── spring-boot/             # Spring Boot アプリ
│   ├── java-ee/                 # Java EE / EJB
│   ├── android/                 # Android バックエンド
│   └── generic/
└── tests/
    └── fixtures/
```

---

## Java固有の課題と変換戦略

### クラス継承 → トレイト + 合成

```java
public abstract class Animal {
    protected String name;
    public abstract String speak();
    public String getName() { return name; }
}

public class Dog extends Animal {
    public Dog(String name) { this.name = name; }
    public String speak() { return "Woof!"; }
}
```

**変換**:
```rust
trait Animal {
    fn name(&self) -> &str;
    fn speak(&self) -> String;
}

struct Dog {
    name: String,
}

impl Dog {
    fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Animal for Dog {
    fn name(&self) -> &str { &self.name }
    fn speak(&self) -> String { "Woof!".to_string() }
}
```

### ガベージコレクション → 所有権

```java
public List<String> processItems(List<String> items) {
    List<String> result = new ArrayList<>();
    for (String item : items) {
        result.add(item.toUpperCase());
    }
    return result;  // GCが元のitemsを回収
}
```

**変換**:
```rust
fn process_items(items: &[String]) -> Vec<String> {
    items.iter().map(|item| item.to_uppercase()).collect()
}
```

### Generics（型消去）→ Rustジェネリクス（単相化）

```java
public class Box<T> {
    private T value;
    public Box(T value) { this.value = value; }
    public T getValue() { return value; }
}
```

**変換**:
```rust
struct Box<T> {
    value: T,
}

impl<T> Box<T> {
    fn new(value: T) -> Self { Self { value } }
    fn value(&self) -> &T { &self.value }
}
```

### 例外 → Result

```java
public String readFile(String path) throws IOException {
    return new String(Files.readAllBytes(Paths.get(path)));
}
```

**変換**:
```rust
fn read_file(path: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(path)
}
```

### synchronized → Mutex/RwLock

```java
public class Counter {
    private int count = 0;
    public synchronized void increment() { count++; }
    public synchronized int getCount() { return count; }
}
```

**変換**:
```rust
use std::sync::Mutex;

struct Counter {
    count: Mutex<i32>,
}

impl Counter {
    fn new() -> Self { Self { count: Mutex::new(0) } }
    fn increment(&self) { *self.count.lock().unwrap() += 1; }
    fn count(&self) -> i32 { *self.count.lock().unwrap() }
}
```

### Spring/JPA アノテーション → Rust相当

| Java (Spring) | Rust 相当 |
|---|---|
| `@RestController` | Axum Router + handler functions |
| `@GetMapping("/path")` | `.route("/path", get(handler))` |
| `@Entity` | SeaORM Entity derive |
| `@Autowired` | 関数引数 or State |
| `@Transactional` | SeaORM transaction |
| `@Scheduled` | tokio::spawn + interval |

---

## バージョン互換

| Java Version | Priority | Notes |
|---|---|---|
| 8 (LTS) | **First** | 企業の35%。モジュールなし。最もシンプル。 |
| 11 (LTS) | **First** | var, HTTP client。2番目に多いLTS。 |
| 17 (LTS) | Second | Records, sealed classes。成長中。 |
| 21 (LTS) | Third | Virtual threads。最新LTS。 |
| 6/7 | Fourth | 銀行に残存。シンプル。 |

### 依存関係の検出

```bash
# Maven
java-to-rust analyze ./project  # pom.xml を解析
# Gradle
java-to-rust analyze ./project  # build.gradle を解析
```

`pom.xml` / `build.gradle` からJavaバージョン、依存ライブラリ、フレームワークを自動検出する。

---

## テストフィクスチャ

```
tests/fixtures/
├── 01_hello_world/
├── 02_class_inheritance/
├── 03_generics/
├── 04_exception_handling/
├── 05_collections/
├── 06_synchronized/
├── 07_stream_api/
├── 08_spring_controller/
├── 09_jpa_entity/
└── 10_small_project/
```

---

## 完了条件

- [ ] `java-to-rust analyze` でJavaプロジェクトの構造レポート出力（Maven/Gradle対応）
- [ ] `java-to-rust convert` でJavaプロジェクトがRustに変換される
- [ ] クラス継承→trait+struct変換が動作する
- [ ] GC依存パターン→所有権変換が動作する
- [ ] 例外→Result変換が動作する
- [ ] Spring Bootプロファイルでアノテーション→Axumハンドラ変換が動作する
- [ ] cargo check + 出力比較の検証ループが動作する
- [ ] Java 8/11のテストフィクスチャで変換成功
- [ ] 全テスト通過、clippy通過

---

## 自律的に動くこと

**あなたは自分で判断して進めてください。**

## 判断原則

1. **Java 8から始める。** 最もシンプルで最も需要が大きい。
2. **フレームワーク非依存を先に。** 純粋なJavaクラス変換を完成させてからSpring等に拡張する。
3. **Javaのnullを排除する。** NullPointerExceptionはJava最大の問題。全てOption<T>に変換する。
4. **php-to-rustと設計を揃える。** LLMプロバイダー、検証ループの設計パターンは共通化する。


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
