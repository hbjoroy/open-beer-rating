use leptos::prelude::*;

#[component]
pub fn ScoreBadge(score: i32) -> impl IntoView {
    let class = match score {
        0..=2 => "score-badge poor",
        3..=4 => "score-badge below-avg",
        5 => "score-badge average",
        6..=7 => "score-badge good",
        8..=9 => "score-badge excellent",
        10 => "score-badge world-class",
        _ => "score-badge",
    };

    view! {
        <span class=class>{format!("{}/10", score)}</span>
    }
}
