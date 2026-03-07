# WordPress 上位50プラグイン DBスキーマ調査

## 概要

RustPressがWordPressプラグインのデータを正しく表示するために必要な
DBスキーマ情報をまとめた調査ドキュメント。

**基本方針**: 既存のWordPressデータベースをそのまま読む（テーブル変更なし）。
各プラグインのデータ形式をRustPressのcompat層で解釈する。

---

## カテゴリ1: ECプラグイン

### 1. WooCommerce (>9M installs)

**カスタムテーブル** (WooCommerce 3.0+):
| テーブル | 用途 |
|---|---|
| `wc_order_stats` | 注文統計（売上集計） |
| `wc_order_product_lookup` | 注文×商品の高速検索 |
| `wc_product_meta_lookup` | 商品メタの高速検索 |
| `wc_customer_lookup` | 顧客統計 |
| `wc_tax_rate_classes` | 税率クラス |
| `woocommerce_sessions` | ゲストセッション（カート） |
| `woocommerce_payment_tokens` | 支払いトークン |
| `woocommerce_shipping_zones` | 配送ゾーン |
| `woocommerce_shipping_zone_methods` | 配送方法 |
| `woocommerce_shipping_zone_locations` | 配送ゾーン地域 |

**wp_posts CPT**:
- `product` — 商品
- `product_variation` — バリエーション商品
- `shop_order` — 注文
- `shop_coupon` — クーポン

**wp_postmeta キー** (`rustpress-commerce/src/woo_compat.rs` に実装済み):
- `_price`, `_regular_price`, `_sale_price`
- `_sku`, `_stock`, `_stock_status`, `_manage_stock`
- `_weight`, `_length`, `_width`, `_height`
- `_virtual`, `_downloadable`, `_product_image_gallery`
- `_tax_status`, `_tax_class`, `_backorders`
- `_billing_*`, `_shipping_*`, `_order_total`, `_order_currency`

**RustPress対応状況**: `rustpress-commerce` クレートで実装済み。
`frontend.rs` の `load_all_postmeta` 経由でテンプレートへ注入。

---

## カテゴリ2: SEOプラグイン

### 2. Yoast SEO (>12M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `yoast_indexable` | 投稿・タームのインデックスメタ（WP CLI経由で生成） |
| `yoast_indexable_hierarchy` | コンテンツ階層 |
| `yoast_migrations` | マイグレーション履歴 |
| `yoast_prominent_words` | 重要ワード分析 |
| `yoast_seo_links` | 内部リンク解析 |

**wp_postmeta キー** (`rustpress-seo/src/yoast_compat.rs` に実装済み):
- `_yoast_wpseo_title` — カスタムSEOタイトル
- `_yoast_wpseo_metadesc` — メタディスクリプション
- `_yoast_wpseo_focuskw` — フォーカスキーワード
- `_yoast_wpseo_canonical` — カノニカルURL
- `_yoast_wpseo_meta-robots-noindex` — noindex
- `_yoast_wpseo_opengraph-title/description/image`
- `_yoast_wpseo_twitter-title/description/image`

**wp_options キー**:
- `wpseo_titles` — 投稿タイプ別タイトルテンプレート
- `wpseo_social` — ソーシャル設定
- `wpseo` — 一般設定

**RustPress対応状況**: `rustpress-seo` クレートで実装済み。
`frontend.rs` の `load_all_postmeta` → `YoastPostSeo::from_meta()` 経由で自動適用。

### 3. Rank Math SEO (~3M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `rank_math_analytics_objects` | アナリティクス対象 |
| `rank_math_analytics_summary` | アナリティクス集計 |
| `rank_math_internal_links` | 内部リンク |
| `rank_math_internal_meta` | 内部SEOメタ |

**wp_postmeta キー**:
- `rank_math_title` — カスタムタイトル
- `rank_math_description` — メタディスクリプション
- `rank_math_focus_keyword` — フォーカスKW
- `rank_math_robots` — robots設定
- `rank_math_canonical_url` — カノニカル
- `rank_math_og_content_image` — OG画像

### 4. AIOSEO (~3M installs)

**カスタムテーブル**:
- `aioseo_posts` — 投稿SEOメタ
- `aioseo_terms` — タームSEOメタ
- `aioseo_notifications` — 通知

**wp_postmeta キー**:
- `_aioseo_title`, `_aioseo_description`, `_aioseo_keywords`

---

## カテゴリ3: フォームプラグイン

### 5. Contact Form 7 (~10M installs)

**カスタムテーブル**: なし（wp_posts + wp_optionsのみ）

**wp_posts CPT**:
- `wpcf7_contact_form` — フォーム定義

**wp_options キー**:
- `wpcf7` — プラグイン設定

**実際のデータ**: フォーム定義は `wp_posts.post_content` にショートコード形式で保存。

**RustPress対応状況**: `rustpress-forms/src/cf7_compat.rs` に実装済み。

### 6. WPForms (~6M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `wpforms_entries` | フォーム送信データ |
| `wpforms_entry_fields` | 送信フィールド値 |
| `wpforms_entry_meta` | 送信メタ |

**wp_posts CPT**:
- `wpforms` — フォーム定義（post_contentにJSON）

### 7. Gravity Forms (~1M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `gf_form` | フォーム定義 |
| `gf_form_meta` | フォームメタ |
| `gf_entry` | 送信エントリ |
| `gf_entry_meta` | エントリメタ |
| `gf_entry_notes` | エントリメモ |
| `gf_form_view` | フォーム表示回数 |

### 8. Ninja Forms (~1M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `nf3_forms` | フォーム定義 |
| `nf3_fields` | フィールド定義 |
| `nf3_field_meta` | フィールドメタ |
| `nf3_actions` | アクション定義 |
| `nf3_action_meta` | アクションメタ |
| `nf3_objects` | サブミット |
| `nf3_object_meta` | サブミットメタ |

---

## カテゴリ4: ページビルダー

### 9. Elementor (~5M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `e_submissions` | フォーム送信（Elementor Forms） |
| `e_submissions_actions_log` | アクションログ |
| `e_globally_used_widgets` | グローバルウィジェット |

**wp_posts CPT**:
- `elementor_library` — テンプレートライブラリ

**wp_postmeta キー**:
- `_elementor_data` — ページウィジェットツリー（JSON、数十〜数百KB）
- `_elementor_css` — 生成済みCSS
- `_elementor_version` — Elementorバージョン
- `_elementor_edit_mode` — 編集モード（"builder"）
- `_elementor_template_type` — テンプレートタイプ

**RustPress対応状況**: `frontend.rs` の `render_elementor_content()` で
heading/text-editor/image/button/icon-list/video/divider/spacerをサポート。

### 10. WPBakery Page Builder (~7M via themes)

**wp_postmeta キー**:
- `_wpb_vc_js_status` — JS最適化ステータス
- `_vc_post_settings` — VC設定（JSON）

**実際のデータ**: コンテンツはshortcode形式 `[vc_row][vc_column]...[/vc_column][/vc_row]`
で `wp_posts.post_content` に保存。

---

## カテゴリ5: カスタムフィールドプラグイン

### 11. ACF – Advanced Custom Fields (~6M installs)

**wp_posts CPT**:
- `acf-field-group` — フィールドグループ定義
- `acf-field` — フィールド定義

**wp_postmeta形式** (`rustpress-fields/src/acf_compat.rs` に実装済み):
```
meta_key: "field_name"     → meta_value: "実際の値"
meta_key: "_field_name"    → meta_value: "field_abc123" (フィールドキー参照)
```

**wp_options キー** (フィールドグループ設定):
- `_acf_version` — ACFバージョン

**RustPress対応状況**: `rustpress-fields` クレートで実装済み。
`frontend.rs` の `load_all_postmeta` → `AcfPostData::from_meta()` 経由でテンプレートへ注入。

### 12. Toolset Types / Pods (~500K installs)

**Toolset wp_postmeta形式**:
- `wpcf-{field_name}` — カスタムフィールド値

**Pods wp_posts CPT**:
- `_pods_pod` — Podグループ定義

---

## カテゴリ6: セキュリティプラグイン

### 13. Wordfence (~5M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `wfblockediplog` | ブロックIPログ |
| `wfblocks7` | ブロックルール |
| `wfconfig` | 設定（key-value） |
| `wfcrawlers` | クローラー情報 |
| `wffilechanges` | ファイル変更検知 |
| `wfhits` | アクセスログ |
| `wfissues` | セキュリティ問題 |
| `wfknownfilelist` | 既知ファイルリスト |
| `wflogins` | ログイン試行ログ |
| `wfls_settings` | ログインセキュリティ設定 |
| `wfnotifications` | 通知 |
| `wfpendingissues` | 保留中の問題 |
| `wfreversecache` | リバースキャッシュ |
| `wfsnipcache` | SNIPキャッシュ |
| `wfstats` | 統計 |
| `wftrafficrates` | トラフィックレート |
| `wfvermetas` | バージョンメタ |

**RustPress対応状況**: `rustpress-security/src/wordfence_compat.rs` に設定互換性実装。

### 14. iThemes Security / Solid Security (~1M installs)

**カスタムテーブル**:
- `itsec_log` — セキュリティイベントログ
- `itsec_lockouts` — ロックアウト記録
- `itsec_temp` — 一時データ

### 15. Loginizer (~1M installs)

**カスタムテーブル**:
- `loginizer_log` — ログイン試行ログ
- `loginizer_brute_force_lock` — ブルートフォースロック

---

## カテゴリ7: 会員・LMSプラグイン

### 16. MemberPress (~300K installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `mepr_transactions` | 決済トランザクション |
| `mepr_subscriptions` | サブスクリプション |
| `mepr_members` | 会員情報 |
| `mepr_events` | イベントログ |

**wp_posts CPT**:
- `memberpressproduct` — 会員プラン
- `memberpressrule` — アクセスルール
- `memberpresscoupon` — クーポン

### 17. LearnDash LMS (~100K installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `learndash_user_activity` | 学習アクティビティ |
| `learndash_user_activity_meta` | アクティビティメタ |

**wp_posts CPT**:
- `sfwd-courses` — コース
- `sfwd-lessons` — レッスン
- `sfwd-topic` — トピック
- `sfwd-quiz` — クイズ
- `sfwd-essays` — エッセイ
- `sfwd-assignment` — 課題

### 18. TutorLMS (~100K installs)

**カスタムテーブル**:
- `tutor_quiz_attempts` — クイズ回答
- `tutor_quiz_attempt_answers` — 回答明細

---

## カテゴリ8: アナリティクス・マーケティング

### 19. MonsterInsights (~3M installs)

**カスタムテーブル**:
- `monsterinsights_summary` — サマリーデータ
- `monsterinsights_popular_posts_summary` — 人気記事

### 20. Site Kit by Google (~3M installs)

**カスタムテーブル**: なし（wp_options に JSON 保存）

**wp_options キー**:
- `googlesitekit_db_version`
- `googlesitekit_settings` — 各モジュール設定
- `googlesitekit_transients` — キャッシュ

---

## カテゴリ9: キャッシュ・パフォーマンス

### 21. WP Rocket (~3M installs via purchase)

**カスタムテーブル**: なし

**wp_options キー**:
- `wp_rocket_settings` — 全設定（JSON）
- `rocket_critical_css` — Critical CSS

### 22. W3 Total Cache (~1M installs)

**カスタムテーブル**: なし（ファイルキャッシュ）

**wp_options キー**:
- `w3tc_config` — 設定

### 23. LiteSpeed Cache (~6M installs)

**カスタムテーブル**: なし

**wp_postmeta キー**:
- `_lscache_vary` — キャッシュバリエーション

---

## カテゴリ10: メディア・画像

### 24. Smush (~1M installs)

**wp_postmeta キー**:
- `_wp_smush_data` — 最適化済み画像データ（JSON）
- `_wp_smush_resize_savings` — リサイズ節約量

### 25. Imagify (~1M installs)

**wp_postmeta キー**:
- `_imagify_data` — 最適化データ（JSON）
- `_imagify_status` — 最適化ステータス
- `_imagify_optimization_level` — 最適化レベル

---

## カテゴリ11: 多言語

### 26. WPML (~1M installs via purchase)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `icl_languages` | 言語定義 |
| `icl_translations` | 翻訳関係（元記事→翻訳） |
| `icl_strings` | 文字列翻訳 |
| `icl_string_translations` | 文字列翻訳値 |
| `icl_string_positions` | 文字列位置 |
| `icl_message_status` | メッセージステータス |
| `icl_mo_files_domains` | MOファイル |

**wp_postmeta キー**:
- `_icl_lang_duplicate_of` — 複製元

### 27. Polylang (~700K installs)

**カスタムテーブル**: なし（wp_termmeta / wp_options を使用）

**wp_termmeta キー**:
- `_pll_language` — タームの言語

**wp_options キー**:
- `polylang` — 設定とマッピング

---

## カテゴリ12: EC拡張・決済

### 28. WooCommerce Subscriptions

**wp_posts CPT**:
- `shop_subscription` — サブスクリプション

**wp_postmeta キー**:
- `_subscription_status`, `_subscription_renewal_order`
- `_billing_interval`, `_billing_period`

### 29. Easy Digital Downloads (~50K installs)

**カスタムテーブル**:
- `edd_orders` — 注文（EDD 3.0+）
- `edd_order_items` — 注文明細
- `edd_order_meta` — 注文メタ
- `edd_customers` — 顧客
- `edd_customer_meta` — 顧客メタ
- `edd_logs` — ログ
- `edd_notes` — メモ

**wp_posts CPT** (EDD 2.x):
- `download` — ダウンロード商品
- `edd_payment` — 支払い

### 30. GiveWP (~100K installs)

**カスタムテーブル**:
- `give_donationmeta` — 寄付メタ
- `give_revenue` — 収益データ

**wp_posts CPT**:
- `give_forms` — 寄付フォーム
- `give_payment` — 支払い

---

## カテゴリ13: コミュニティ・SNS

### 31. BuddyPress (~200K installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `bp_activity` | アクティビティ |
| `bp_activity_meta` | アクティビティメタ |
| `bp_friends` | フレンド関係 |
| `bp_groups` | グループ |
| `bp_groups_groupmeta` | グループメタ |
| `bp_groups_members` | グループメンバー |
| `bp_messages_messages` | メッセージ |
| `bp_messages_recipients` | 受信者 |
| `bp_messages_meta` | メッセージメタ |
| `bp_notifications` | 通知 |
| `bp_user_blogs` | ユーザーブログ |
| `bp_xprofile_data` | プロフィールデータ |
| `bp_xprofile_fields` | プロフィールフィールド |
| `bp_xprofile_groups` | プロフィールグループ |
| `bp_xprofile_meta` | プロフィールメタ |

### 32. bbPress (~400K installs)

**wp_posts CPT**:
- `forum` — フォーラム
- `topic` — トピック
- `reply` — 返信

**wp_terms taxonomy**:
- `forum` — フォーラムカテゴリ

---

## カテゴリ14: バックアップ・移行

### 33. UpdraftPlus (~3M installs)

**カスタムテーブル**: なし

**wp_options キー**:
- `updraft_*` — バックアップ設定

### 34. All-in-One WP Migration (~5M installs)

**カスタムテーブル**: なし

---

## カテゴリ15: テーブル・データ表示

### 35. TablePress (~800K installs)

**wp_posts CPT**:
- `tablepress_table` — テーブル定義（post_contentにJSON）

**wp_postmeta キー**:
- `_tablepress_table_data` — テーブルデータ（JSON）

---

## カテゴリ16: リダイレクト・SEO補助

### 36. Redirection (~2M installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `redirection_items` | リダイレクトルール |
| `redirection_groups` | グループ |
| `redirection_logs` | アクセスログ |
| `redirection_404` | 404ログ |

---

## カテゴリ17: イベント

### 37. The Events Calendar (~900K installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `tribe_events` | イベントカレンダー（廃止予定） |

**wp_posts CPT**:
- `tribe_events` — イベント
- `tribe_venue` — 会場
- `tribe_organizer` — 主催者

**wp_postmeta キー**:
- `_EventStartDate`, `_EventEndDate`
- `_EventTimezone`, `_EventAllDay`
- `_EventCost`, `_EventURL`

---

## カテゴリ18: スライダー・ビジュアル

### 38. Slider Revolution (~7M via themes)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `revslider_sliders` | スライダー定義 |
| `revslider_slides` | スライド |
| `revslider_static_slides` | 静的スライド |
| `revslider_css` | カスタムCSS |
| `revslider_layer_animations` | アニメーション |
| `revslider_navigations` | ナビゲーション |

---

## カテゴリ19: アフィリエイト・マーケティング

### 39. AffiliateWP (~70K installs)

**カスタムテーブル**:
| テーブル | 用途 |
|---|---|
| `affiliate_wp_affiliates` | アフィリエイター |
| `affiliate_wp_affiliatemeta` | アフィリエイターメタ |
| `affiliate_wp_referrals` | 紹介 |
| `affiliate_wp_referralmeta` | 紹介メタ |
| `affiliate_wp_clicks` | クリック |
| `affiliate_wp_visits` | 訪問 |
| `affiliate_wp_payouts` | 支払い |

---

## RustPress実装状況サマリ

| プラグイン | 互換レイヤー | フロントエンド統合 |
|---|---|---|
| WooCommerce | `rustpress-commerce/woo_compat.rs` | ✅ `load_all_postmeta` 経由 |
| Yoast SEO | `rustpress-seo/yoast_compat.rs` | ✅ `YoastPostSeo::from_meta()` 経由 |
| Contact Form 7 | `rustpress-forms/cf7_compat.rs` | ✅ 実装済み |
| ACF | `rustpress-fields/acf_compat.rs` | ✅ `AcfPostData::from_meta()` 経由 |
| Elementor | `frontend.rs render_elementor_content()` | ✅ 主要ウィジェット対応 |
| Wordfence | `rustpress-security/wordfence_compat.rs` | — 管理画面のみ |
| Rank Math | 未実装 | — 要実装 |
| AIOSEO | 未実装 | — 要実装 |
| WPForms | `rustpress-forms` | — DB統合要実装 |
| Gravity Forms | 未実装 | — 要実装 |
| WPML | 未実装 | — 多言語対応は別タスク |
| Polylang | 未実装 | — 多言語対応は別タスク |
| BuddyPress | 未実装 | — コミュニティ機能は別タスク |
| その他 | — | wp_postmetaはall_meta経由でテンプレートアクセス可 |

## テンプレートでのアクセス方法 (Tera)

```html
{# ACF カスタムフィールド #}
{% if acf_fields %}
  <div class="hero-title">{{ acf_fields.hero_title }}</div>
  <div class="hero_image">{{ acf_fields.hero_image }}</div>
{% endif %}

{# WooCommerce 商品 #}
{% if product %}
  <span class="price">¥{{ product.price }}</span>
  <span class="sku">SKU: {{ product.sku }}</span>
  <span class="stock">{{ product.stock_status }}</span>
{% endif %}

{# wp_postmetaへの直接アクセス #}
{% if post_meta._my_custom_key %}
  {{ post_meta._my_custom_key }}
{% endif %}

{# SEO情報はauto-inject済み (seo_meta_tags) #}
{{ seo_meta_tags | safe }}
```

---

*調査日: 2026-03-08 — #02 API/移行担当*
