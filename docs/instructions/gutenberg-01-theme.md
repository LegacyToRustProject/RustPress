# Gutenberg統合 — #01 テーマ担当の作業

## 概要

現在の `post-edit.html` にある独自バニラJS実装（約1000行）を削除し、
WordPress本体のGutenberg JSアセット（React）を配信してブロックエディタを動かす。

## 前提

- Gutenbergが呼ぶREST APIは#02が対応（別指示書）
- WordPress 6.7のGutenbergアセットをそのまま使用（GPL v2、ライセンス互換）
- RustPressのDockerコンテナ内WordPressからアセットを取得可能

## 作業手順

### Step 1: WordPressからGutenbergアセットを取得

```bash
# WordPressコンテナからアセットをコピー
docker cp rustpress-1_wordpress_1:/var/www/html/wp-includes/js/dist/ static/wp-includes/js/dist/
docker cp rustpress-1_wordpress_1:/var/www/html/wp-includes/js/dist/vendor/ static/wp-includes/js/dist/vendor/
docker cp rustpress-1_wordpress_1:/var/www/html/wp-includes/css/dist/ static/wp-includes/css/dist/
```

必要なファイル（.min.js のみ、非minは不要）:

**vendor（必須）:**
- react.min.js
- react-dom.min.js
- react-jsx-runtime.min.js
- regenerator-runtime.min.js
- wp-polyfill.min.js

**Gutenberg コア（必須）:**
- element.min.js
- hooks.min.js
- i18n.min.js
- data.min.js
- dom-ready.min.js
- dom.min.js
- url.min.js
- api-fetch.min.js
- escape-html.min.js
- html-entities.min.js
- is-shallow-equal.min.js
- compose.min.js
- keycodes.min.js
- primitives.min.js
- rich-text.min.js
- deprecated.min.js
- token-list.min.js
- blob.min.js
- shortcode.min.js
- autop.min.js
- blocks.min.js
- components.min.js
- notices.min.js
- priority-queue.min.js
- redux-routine.min.js
- data-controls.min.js
- plugins.min.js
- core-data.min.js
- block-serialization-default-parser.min.js
- block-editor.min.js
- block-library.min.js
- editor.min.js
- edit-post.min.js
- format-library.min.js
- keyboard-shortcuts.min.js
- media-utils.min.js
- viewport.min.js
- preferences.min.js
- preferences-persistence.min.js
- private-apis.min.js
- style-engine.min.js
- annotations.min.js
- wordcount.min.js

**CSS（必須）:**
- block-editor/style.min.css
- block-library/style.min.css
- block-library/editor.min.css
- components/style.min.css
- edit-post/style.min.css
- editor/style.min.css
- format-library/style.min.css
- nux/style.min.css

### Step 2: post-edit.html を書き換え

現在の独自実装（`<script>` 内の約1000行）を削除し、以下に置き換え:

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{% if editing %}Edit Post{% else %}Add New Post{% endif %} - {{ site_name | default(value="RustPress") }}</title>

    <!-- Gutenberg CSS -->
    <link rel="stylesheet" href="/wp-includes/css/dist/components/style.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/block-editor/style.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/block-library/style.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/block-library/editor.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/editor/style.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/edit-post/style.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/format-library/style.min.css">
    <link rel="stylesheet" href="/wp-includes/css/dist/nux/style.min.css">

    <style>
        body { margin: 0; }
        #editor { height: 100vh; }
    </style>
</head>
<body>
    {{ wp_nonce_field(action="wpnonce_save_post") | safe }}

    <div id="editor"></div>

    <!-- Vendor -->
    <script src="/wp-includes/js/dist/vendor/react.min.js"></script>
    <script src="/wp-includes/js/dist/vendor/react-dom.min.js"></script>
    <script src="/wp-includes/js/dist/vendor/react-jsx-runtime.min.js"></script>
    <script src="/wp-includes/js/dist/vendor/regenerator-runtime.min.js"></script>

    <!-- WordPress globals setup -->
    <script>
    // Gutenberg expects these globals
    window.wp = window.wp || {};

    // api-fetch needs the REST API root and nonce
    window.wpApiSettings = {
        root: '{{ site_url | default(value="") }}/wp-json/',
        nonce: '{{ api_nonce | default(value="") }}',
        versionString: 'wp/v2/'
    };

    // Editor settings passed from server
    window._wpEditorSettings = {
        post: {
            id: {{ post_id | default(value=0) }},
            type: '{{ post_type | default(value="post") }}',
            title: {{ post_title_json | default(value="\"\"") | safe }},
            content: {{ post_content_json | default(value="\"\"") | safe }},
            excerpt: {{ post_excerpt_json | default(value="\"\"") | safe }},
            status: '{{ post_status | default(value="draft") }}',
            slug: '{{ post_slug | default(value="") }}',
        },
        siteUrl: '{{ site_url | default(value="") }}',
    };
    </script>

    <!-- Gutenberg packages (order matters — dependencies first) -->
    <script src="/wp-includes/js/dist/hooks.min.js"></script>
    <script src="/wp-includes/js/dist/i18n.min.js"></script>
    <script src="/wp-includes/js/dist/element.min.js"></script>
    <script src="/wp-includes/js/dist/is-shallow-equal.min.js"></script>
    <script src="/wp-includes/js/dist/priority-queue.min.js"></script>
    <script src="/wp-includes/js/dist/redux-routine.min.js"></script>
    <script src="/wp-includes/js/dist/data.min.js"></script>
    <script src="/wp-includes/js/dist/data-controls.min.js"></script>
    <script src="/wp-includes/js/dist/deprecated.min.js"></script>
    <script src="/wp-includes/js/dist/dom-ready.min.js"></script>
    <script src="/wp-includes/js/dist/dom.min.js"></script>
    <script src="/wp-includes/js/dist/url.min.js"></script>
    <script src="/wp-includes/js/dist/escape-html.min.js"></script>
    <script src="/wp-includes/js/dist/html-entities.min.js"></script>
    <script src="/wp-includes/js/dist/shortcode.min.js"></script>
    <script src="/wp-includes/js/dist/autop.min.js"></script>
    <script src="/wp-includes/js/dist/api-fetch.min.js"></script>
    <script src="/wp-includes/js/dist/keycodes.min.js"></script>
    <script src="/wp-includes/js/dist/compose.min.js"></script>
    <script src="/wp-includes/js/dist/token-list.min.js"></script>
    <script src="/wp-includes/js/dist/blob.min.js"></script>
    <script src="/wp-includes/js/dist/primitives.min.js"></script>
    <script src="/wp-includes/js/dist/rich-text.min.js"></script>
    <script src="/wp-includes/js/dist/notices.min.js"></script>
    <script src="/wp-includes/js/dist/wordcount.min.js"></script>
    <script src="/wp-includes/js/dist/style-engine.min.js"></script>
    <script src="/wp-includes/js/dist/private-apis.min.js"></script>
    <script src="/wp-includes/js/dist/preferences-persistence.min.js"></script>
    <script src="/wp-includes/js/dist/preferences.min.js"></script>
    <script src="/wp-includes/js/dist/viewport.min.js"></script>
    <script src="/wp-includes/js/dist/plugins.min.js"></script>
    <script src="/wp-includes/js/dist/annotations.min.js"></script>
    <script src="/wp-includes/js/dist/keyboard-shortcuts.min.js"></script>
    <script src="/wp-includes/js/dist/components.min.js"></script>
    <script src="/wp-includes/js/dist/block-serialization-default-parser.min.js"></script>
    <script src="/wp-includes/js/dist/blocks.min.js"></script>
    <script src="/wp-includes/js/dist/media-utils.min.js"></script>
    <script src="/wp-includes/js/dist/core-data.min.js"></script>
    <script src="/wp-includes/js/dist/block-editor.min.js"></script>
    <script src="/wp-includes/js/dist/block-library.min.js"></script>
    <script src="/wp-includes/js/dist/editor.min.js"></script>
    <script src="/wp-includes/js/dist/edit-post.min.js"></script>
    <script src="/wp-includes/js/dist/format-library.min.js"></script>

    <!-- Initialize the editor -->
    <script>
    (function() {
        var settings = window._wpEditorSettings;

        // Initialize api-fetch with the REST root
        wp.apiFetch.use(wp.apiFetch.createRootURLMiddleware(wpApiSettings.root));
        wp.apiFetch.use(wp.apiFetch.createNonceMiddleware(wpApiSettings.nonce));

        // Register core blocks
        wp.blockLibrary.registerCoreBlocks();

        // Initialize the editor
        var postId = settings.post.id;
        if (postId && postId > 0) {
            // Editing existing post
            wp.editPost.initializeEditor('editor', settings.post.type, postId);
        } else {
            // New post — create via API first, then init editor
            wp.apiFetch({
                path: '/wp/v2/' + settings.post.type + 's',
                method: 'POST',
                data: {
                    title: '',
                    content: '',
                    status: 'auto-draft',
                },
            }).then(function(newPost) {
                wp.editPost.initializeEditor('editor', settings.post.type, newPost.id);
            });
        }
    })();
    </script>
</body>
</html>
```

### Step 3: Rustハンドラの修正

`wp_admin.rs` の `post_editor_new()` / `post_editor_edit()` が返すテンプレート変数を調整:

- `post_id`: 投稿ID（新規の場合は0）
- `post_type`: "post" or "page"
- `post_title_json`: JSONエンコードされたタイトル
- `post_content_json`: JSONエンコードされたコンテンツ
- `post_excerpt_json`: JSONエンコードされたエキサプト
- `post_status`: "draft" / "publish" 等
- `post_slug`: スラッグ
- `site_url`: サイトURL
- `api_nonce`: REST API用のnonce

### Step 4: 静的ファイル配信の確認

`rustpress-server` が `/wp-includes/` パスの静的ファイルを配信できることを確認。
既存の static file ハンドラに `/wp-includes/` を追加する必要があるかもしれない。

### Step 5: .gitignore にアセットを追加しない

GutenbergアセットはWordPressのGPL v2コードなので、リポジトリに含めてよい。
ただしサイズが大きい（min.jsだけで約5.5MB）場合は:
- Dockerビルド時にコピーするスクリプトを用意する
- または `static/wp-includes/` をgitに含める（推奨: 配布しやすい）

## 完了条件

- [ ] `post-edit.html` から独自JS実装を削除
- [ ] WordPress Gutenberg JS/CSSが `static/wp-includes/` に配置
- [ ] ブラウザで `/wp-admin/post-new.php` にアクセスするとGutenbergエディタが表示
- [ ] `cargo check --workspace` / `cargo test` が通る
- [ ] ブロックの追加・編集・保存が動作する（#02のAPI対応と連携）

## 注意事項

- 独自実装の `post-edit.html` は `post-edit.html.bak` としてバックアップを残してよい
- Gutenbergの動作には#02のAPI修正が必要な場合がある。ブラウザのDevToolsでAPIエラーを確認して #02 に報告すること
- WordPress 6.7 のアセットを使用すること（Dockerコンテナと同じバージョン）
