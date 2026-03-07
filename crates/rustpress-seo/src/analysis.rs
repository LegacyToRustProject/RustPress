//! SEO content analysis — keyword density, readability scoring,
//! and on-page SEO recommendations.
//!
//! Provides Yoast/RankMath-style content analysis for posts.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Overall SEO score level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeoScore {
    Good,
    Ok,
    NeedsImprovement,
}

/// A single recommendation from the analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoRecommendation {
    pub category: String,
    pub score: SeoScore,
    pub message: String,
}

/// Result of a complete SEO analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub overall_score: SeoScore,
    pub keyword_density: f64,
    pub word_count: usize,
    pub readability_score: f64,
    pub recommendations: Vec<SeoRecommendation>,
}

/// Input for SEO analysis.
pub struct AnalysisInput<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub meta_description: &'a str,
    pub focus_keyword: &'a str,
    pub slug: &'a str,
}

/// Perform a complete SEO analysis on the given content.
pub fn analyze(input: &AnalysisInput) -> AnalysisResult {
    let clean_content = strip_html(input.content);
    let word_count = count_words(&clean_content);
    let keyword_density = calculate_keyword_density(&clean_content, input.focus_keyword);
    let readability_score = calculate_readability(&clean_content);

    let mut recommendations = Vec::new();

    // Title analysis
    analyze_title(input, &mut recommendations);

    // Meta description analysis
    analyze_meta_description(input, &mut recommendations);

    // Keyword analysis
    analyze_keyword(input, &clean_content, keyword_density, &mut recommendations);

    // Content length analysis
    analyze_content_length(word_count, &mut recommendations);

    // Readability analysis
    analyze_readability_score(readability_score, &mut recommendations);

    // Slug analysis
    analyze_slug(input, &mut recommendations);

    // Heading analysis
    analyze_headings(input.content, input.focus_keyword, &mut recommendations);

    let overall_score = calculate_overall_score(&recommendations);

    AnalysisResult {
        overall_score,
        keyword_density,
        word_count,
        readability_score,
        recommendations,
    }
}

fn analyze_title(input: &AnalysisInput, recs: &mut Vec<SeoRecommendation>) {
    let title_len = input.title.len();

    if title_len == 0 {
        recs.push(SeoRecommendation {
            category: "Title".into(),
            score: SeoScore::NeedsImprovement,
            message: "Page title is missing.".into(),
        });
        return;
    }

    if title_len < 30 {
        recs.push(SeoRecommendation {
            category: "Title".into(),
            score: SeoScore::Ok,
            message: format!("Title is too short ({title_len} chars). Aim for 50-60 characters."),
        });
    } else if title_len > 60 {
        recs.push(SeoRecommendation {
            category: "Title".into(),
            score: SeoScore::Ok,
            message: format!(
                "Title is too long ({title_len} chars). Keep it under 60 characters to avoid truncation in search results."
            ),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Title".into(),
            score: SeoScore::Good,
            message: format!("Title length is good ({title_len} chars)."),
        });
    }

    if !input.focus_keyword.is_empty()
        && !input
            .title
            .to_lowercase()
            .contains(&input.focus_keyword.to_lowercase())
    {
        recs.push(SeoRecommendation {
            category: "Title".into(),
            score: SeoScore::NeedsImprovement,
            message: "Focus keyword not found in the title.".into(),
        });
    }
}

fn analyze_meta_description(input: &AnalysisInput, recs: &mut Vec<SeoRecommendation>) {
    let desc_len = input.meta_description.len();

    if desc_len == 0 {
        recs.push(SeoRecommendation {
            category: "Meta Description".into(),
            score: SeoScore::NeedsImprovement,
            message: "Meta description is missing.".into(),
        });
        return;
    }

    if desc_len < 120 {
        recs.push(SeoRecommendation {
            category: "Meta Description".into(),
            score: SeoScore::Ok,
            message: format!(
                "Meta description is short ({desc_len} chars). Aim for 150-160 characters."
            ),
        });
    } else if desc_len > 160 {
        recs.push(SeoRecommendation {
            category: "Meta Description".into(),
            score: SeoScore::Ok,
            message: format!(
                "Meta description is too long ({desc_len} chars). Keep it under 160 characters."
            ),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Meta Description".into(),
            score: SeoScore::Good,
            message: format!("Meta description length is good ({desc_len} chars)."),
        });
    }

    if !input.focus_keyword.is_empty()
        && !input
            .meta_description
            .to_lowercase()
            .contains(&input.focus_keyword.to_lowercase())
    {
        recs.push(SeoRecommendation {
            category: "Meta Description".into(),
            score: SeoScore::Ok,
            message: "Focus keyword not found in the meta description.".into(),
        });
    }
}

fn analyze_keyword(
    input: &AnalysisInput,
    clean_content: &str,
    density: f64,
    recs: &mut Vec<SeoRecommendation>,
) {
    if input.focus_keyword.is_empty() {
        recs.push(SeoRecommendation {
            category: "Keyword".into(),
            score: SeoScore::Ok,
            message: "No focus keyword set.".into(),
        });
        return;
    }

    if density < 0.5 {
        recs.push(SeoRecommendation {
            category: "Keyword Density".into(),
            score: SeoScore::NeedsImprovement,
            message: format!("Keyword density is low ({density:.1}%). Aim for 1-3%."),
        });
    } else if density > 3.0 {
        recs.push(SeoRecommendation {
            category: "Keyword Density".into(),
            score: SeoScore::NeedsImprovement,
            message: format!(
                "Keyword density is too high ({density:.1}%). This may look like keyword stuffing. Aim for 1-3%."
            ),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Keyword Density".into(),
            score: SeoScore::Good,
            message: format!("Keyword density is good ({density:.1}%)."),
        });
    }

    // Check keyword in first paragraph
    let first_para = clean_content
        .split("\n\n")
        .next()
        .unwrap_or("")
        .to_lowercase();
    if first_para.contains(&input.focus_keyword.to_lowercase()) {
        recs.push(SeoRecommendation {
            category: "Keyword".into(),
            score: SeoScore::Good,
            message: "Focus keyword appears in the first paragraph.".into(),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Keyword".into(),
            score: SeoScore::Ok,
            message: "Focus keyword not found in the first paragraph.".into(),
        });
    }
}

fn analyze_content_length(word_count: usize, recs: &mut Vec<SeoRecommendation>) {
    if word_count < 300 {
        recs.push(SeoRecommendation {
            category: "Content Length".into(),
            score: SeoScore::NeedsImprovement,
            message: format!(
                "Content is too short ({word_count} words). Aim for at least 300 words."
            ),
        });
    } else if word_count < 600 {
        recs.push(SeoRecommendation {
            category: "Content Length".into(),
            score: SeoScore::Ok,
            message: format!(
                "Content length is acceptable ({word_count} words). Longer content (1000+) tends to rank better."
            ),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Content Length".into(),
            score: SeoScore::Good,
            message: format!("Content length is good ({word_count} words)."),
        });
    }
}

fn analyze_readability_score(score: f64, recs: &mut Vec<SeoRecommendation>) {
    if score >= 60.0 {
        recs.push(SeoRecommendation {
            category: "Readability".into(),
            score: SeoScore::Good,
            message: format!("Readability score is good ({score:.0}). Content is easy to read."),
        });
    } else if score >= 30.0 {
        recs.push(SeoRecommendation {
            category: "Readability".into(),
            score: SeoScore::Ok,
            message: format!(
                "Readability score is average ({score:.0}). Consider using shorter sentences."
            ),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Readability".into(),
            score: SeoScore::NeedsImprovement,
            message: format!(
                "Readability score is low ({score:.0}). Use shorter sentences and simpler words."
            ),
        });
    }
}

fn analyze_slug(input: &AnalysisInput, recs: &mut Vec<SeoRecommendation>) {
    if input.slug.is_empty() {
        return;
    }

    if !input.focus_keyword.is_empty() {
        let keyword_slug = input.focus_keyword.to_lowercase().replace(' ', "-");
        if input.slug.contains(&keyword_slug) {
            recs.push(SeoRecommendation {
                category: "URL".into(),
                score: SeoScore::Good,
                message: "Focus keyword found in the URL slug.".into(),
            });
        } else {
            recs.push(SeoRecommendation {
                category: "URL".into(),
                score: SeoScore::Ok,
                message: "Focus keyword not found in the URL slug.".into(),
            });
        }
    }

    if input.slug.len() > 75 {
        recs.push(SeoRecommendation {
            category: "URL".into(),
            score: SeoScore::Ok,
            message: "URL slug is quite long. Shorter URLs tend to perform better.".into(),
        });
    }
}

fn analyze_headings(html_content: &str, focus_keyword: &str, recs: &mut Vec<SeoRecommendation>) {
    let h_re = Regex::new(r"(?i)<h[2-6][^>]*>(.*?)</h[2-6]>").unwrap();
    let headings: Vec<String> = h_re
        .captures_iter(html_content)
        .map(|c| strip_html(c.get(1).map_or("", |m| m.as_str())))
        .collect();

    if headings.is_empty() {
        recs.push(SeoRecommendation {
            category: "Headings".into(),
            score: SeoScore::Ok,
            message: "No subheadings (H2-H6) found. Consider adding subheadings to structure your content.".into(),
        });
    } else {
        recs.push(SeoRecommendation {
            category: "Headings".into(),
            score: SeoScore::Good,
            message: format!(
                "Found {} subheading(s). Good content structure.",
                headings.len()
            ),
        });

        if !focus_keyword.is_empty() {
            let kw_lower = focus_keyword.to_lowercase();
            let has_keyword = headings
                .iter()
                .any(|h| h.to_lowercase().contains(&kw_lower));
            if has_keyword {
                recs.push(SeoRecommendation {
                    category: "Headings".into(),
                    score: SeoScore::Good,
                    message: "Focus keyword found in a subheading.".into(),
                });
            } else {
                recs.push(SeoRecommendation {
                    category: "Headings".into(),
                    score: SeoScore::Ok,
                    message: "Focus keyword not found in any subheading.".into(),
                });
            }
        }
    }
}

fn calculate_overall_score(recs: &[SeoRecommendation]) -> SeoScore {
    if recs.is_empty() {
        return SeoScore::Ok;
    }

    let needs_improvement = recs
        .iter()
        .filter(|r| r.score == SeoScore::NeedsImprovement)
        .count();

    let good = recs.iter().filter(|r| r.score == SeoScore::Good).count();

    if needs_improvement >= 3 {
        SeoScore::NeedsImprovement
    } else if good > recs.len() / 2 {
        SeoScore::Good
    } else {
        SeoScore::Ok
    }
}

/// Strip HTML tags from content.
fn strip_html(html: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    let stripped = re.replace_all(html, "");
    let ws = Regex::new(r"\s+").unwrap();
    ws.replace_all(&stripped, " ").trim().to_string()
}

/// Count words in plain text.
fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Calculate keyword density as a percentage.
fn calculate_keyword_density(text: &str, keyword: &str) -> f64 {
    if keyword.is_empty() || text.is_empty() {
        return 0.0;
    }

    let text_lower = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();
    let word_count = count_words(&text_lower);

    if word_count == 0 {
        return 0.0;
    }

    let keyword_words = count_words(&keyword_lower);
    let mut occurrences = 0;
    let text_words: Vec<String> = text_lower
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| !w.is_empty())
        .collect();

    if keyword_words == 1 {
        occurrences = text_words.iter().filter(|w| *w == &keyword_lower).count();
    } else {
        // Multi-word keyword: count phrase occurrences
        let kw_words: Vec<&str> = keyword_lower.split_whitespace().collect();
        for window in text_words.windows(keyword_words) {
            let matches = window.iter().zip(kw_words.iter()).all(|(a, b)| a == b);
            if matches {
                occurrences += 1;
            }
        }
    }

    (occurrences as f64 / word_count as f64) * 100.0
}

/// Calculate Flesch Reading Ease score.
/// Higher scores = easier to read (0-100 scale).
fn calculate_readability(text: &str) -> f64 {
    let sentences = count_sentences(text);
    let words = count_words(text);
    let syllables = count_syllables(text);

    if words == 0 || sentences == 0 {
        return 0.0;
    }

    let avg_sentence_length = words as f64 / sentences as f64;
    let avg_syllables_per_word = syllables as f64 / words as f64;

    // Flesch Reading Ease formula
    let score = 206.835 - (1.015 * avg_sentence_length) - (84.6 * avg_syllables_per_word);

    score.clamp(0.0, 100.0)
}

fn count_sentences(text: &str) -> usize {
    let re = Regex::new(r"[.!?]+\s").unwrap();
    let count = re.find_iter(text).count();
    // At least 1 sentence if there's content
    if count == 0 && !text.trim().is_empty() {
        1
    } else {
        count
    }
}

fn count_syllables(text: &str) -> usize {
    text.split_whitespace().map(count_word_syllables).sum()
}

fn count_word_syllables(word: &str) -> usize {
    let word = word.to_lowercase();
    let word = word.trim_matches(|c: char| !c.is_alphabetic());

    if word.is_empty() {
        return 0;
    }

    if word.len() <= 3 {
        return 1;
    }

    let vowels = ['a', 'e', 'i', 'o', 'u', 'y'];
    let mut count = 0;
    let mut prev_vowel = false;

    for ch in word.chars() {
        let is_vowel = vowels.contains(&ch);
        if is_vowel && !prev_vowel {
            count += 1;
        }
        prev_vowel = is_vowel;
    }

    // Silent 'e' at end
    if word.ends_with('e') && count > 1 {
        count -= 1;
    }

    count.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_analysis() {
        let content = "This is a test article about Rust programming. ".repeat(30);
        let input = AnalysisInput {
            title: "Getting Started with Rust Programming",
            content: &content,
            meta_description: "Learn how to get started with Rust programming language in this comprehensive guide.",
            focus_keyword: "rust programming",
            slug: "getting-started-with-rust-programming",
        };

        let result = analyze(&input);
        assert!(result.word_count > 100);
        assert!(result.keyword_density > 0.0);
        assert!(!result.recommendations.is_empty());
    }

    #[test]
    fn test_keyword_density_single_word() {
        let text = "rust is great. I love rust. rust is fast.";
        let density = calculate_keyword_density(text, "rust");
        // 3 occurrences in ~9 words ≈ 33%
        assert!(density > 20.0);
    }

    #[test]
    fn test_keyword_density_multi_word() {
        let text = "I love rust programming. Rust programming is great. Learn more about coding.";
        let density = calculate_keyword_density(text, "rust programming");
        assert!(density > 0.0);
    }

    #[test]
    fn test_keyword_density_no_keyword() {
        let density = calculate_keyword_density("hello world", "");
        assert_eq!(density, 0.0);
    }

    #[test]
    fn test_word_count() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("  spaces  everywhere  "), 2);
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn test_strip_html() {
        assert_eq!(
            strip_html("<p>Hello <strong>world</strong></p>"),
            "Hello world"
        );
        assert_eq!(strip_html("No HTML here"), "No HTML here");
    }

    #[test]
    fn test_syllable_counting() {
        assert_eq!(count_word_syllables("the"), 1);
        assert_eq!(count_word_syllables("hello"), 2);
        assert_eq!(count_word_syllables("beautiful"), 3);
        assert_eq!(count_word_syllables("a"), 1);
    }

    #[test]
    fn test_readability_simple_text() {
        let text = "This is a short sentence. This is another one. Simple words are good.";
        let score = calculate_readability(text);
        assert!(
            score > 50.0,
            "Simple text should have high readability: {score}"
        );
    }

    #[test]
    fn test_title_analysis_good_length() {
        let input = AnalysisInput {
            title: "How to Learn Rust Programming Language Quickly",
            content: &"word ".repeat(300),
            meta_description: "A guide to learning Rust programming.",
            focus_keyword: "rust programming",
            slug: "learn-rust-programming",
        };

        let result = analyze(&input);
        let title_recs: Vec<_> = result
            .recommendations
            .iter()
            .filter(|r| r.category == "Title")
            .collect();
        assert!(title_recs.iter().any(|r| r.score == SeoScore::Good));
    }

    #[test]
    fn test_missing_meta_description() {
        let input = AnalysisInput {
            title: "Test",
            content: &"word ".repeat(300),
            meta_description: "",
            focus_keyword: "test",
            slug: "test",
        };

        let result = analyze(&input);
        let desc_recs: Vec<_> = result
            .recommendations
            .iter()
            .filter(|r| r.category == "Meta Description")
            .collect();
        assert!(desc_recs
            .iter()
            .any(|r| r.score == SeoScore::NeedsImprovement));
    }

    #[test]
    fn test_short_content_warning() {
        let input = AnalysisInput {
            title: "Short Post",
            content: "This is very short.",
            meta_description: "Short",
            focus_keyword: "short",
            slug: "short",
        };

        let result = analyze(&input);
        assert!(result.word_count < 300);
        let content_recs: Vec<_> = result
            .recommendations
            .iter()
            .filter(|r| r.category == "Content Length")
            .collect();
        assert!(content_recs
            .iter()
            .any(|r| r.score == SeoScore::NeedsImprovement));
    }

    #[test]
    fn test_heading_analysis() {
        let content = "<h2>Introduction to Rust</h2><p>Rust is great.</p><h3>Why Rust?</h3><p>Performance.</p>";
        let mut recs = Vec::new();
        analyze_headings(content, "rust", &mut recs);

        assert!(recs
            .iter()
            .any(|r| r.category == "Headings" && r.score == SeoScore::Good));
    }

    #[test]
    fn test_slug_with_keyword() {
        let input = AnalysisInput {
            title: "Rust Guide",
            content: &"word ".repeat(300),
            meta_description: "A Rust guide",
            focus_keyword: "rust guide",
            slug: "rust-guide",
        };

        let result = analyze(&input);
        let slug_recs: Vec<_> = result
            .recommendations
            .iter()
            .filter(|r| r.category == "URL")
            .collect();
        assert!(slug_recs.iter().any(|r| r.score == SeoScore::Good));
    }

    #[test]
    fn test_overall_score_good() {
        let recs = vec![
            SeoRecommendation {
                category: "A".into(),
                score: SeoScore::Good,
                message: String::new(),
            },
            SeoRecommendation {
                category: "B".into(),
                score: SeoScore::Good,
                message: String::new(),
            },
            SeoRecommendation {
                category: "C".into(),
                score: SeoScore::Ok,
                message: String::new(),
            },
        ];
        assert_eq!(calculate_overall_score(&recs), SeoScore::Good);
    }

    #[test]
    fn test_overall_score_needs_improvement() {
        let recs = vec![
            SeoRecommendation {
                category: "A".into(),
                score: SeoScore::NeedsImprovement,
                message: String::new(),
            },
            SeoRecommendation {
                category: "B".into(),
                score: SeoScore::NeedsImprovement,
                message: String::new(),
            },
            SeoRecommendation {
                category: "C".into(),
                score: SeoScore::NeedsImprovement,
                message: String::new(),
            },
        ];
        assert_eq!(calculate_overall_score(&recs), SeoScore::NeedsImprovement);
    }
}
