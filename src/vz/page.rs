use super::*;

const PAGE_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <title>{title}</title>
    <style>
        {style}
    </style>
</head>
<body>
    <div class="cards">
        {cards}
    </div>
</body>
</html>
"#;

const STYLE_TEMPLATE: &str = r#"
body {
    background-color: #323232;
    color: white;
    margin: 0 auto;
}
.cards {
    display: flex;
    flex-wrap: wrap;
}
.card {
    display: flex;
    flex-direction: column;
    padding: 10px;
}
.card-title {
    margin: 4px;
}
"#;

const CARD_TEMPLATE: &str = r#"
<div class="card">
    <code class="card-title">{name}</code>
    <img src="{image}" alt="{name}" />
</div>
"#;

#[derive(Serialize)]
struct Page {
    title: String,
    style: String,
    cards: String,
}

#[derive(Serialize)]
struct Card {
    name: String,
    image: String,
}

pub struct Builder {
    title: String,
    cards: Vec<Card>,
}

impl Builder {
    pub fn new(title: impl ToString) -> Self {
        Self {
            title: title.to_string(),
            cards: vec![],
        }
    }

    pub fn push_card(&mut self, name: impl ToString, image: impl ToString) {
        self.cards.push(Card {
            name: name.to_string(),
            image: image.to_string(),
        });
    }

    pub fn build(self) -> Result<String> {
        use tinytemplate::TinyTemplate;

        // Prepare template.
        let mut tt = TinyTemplate::new();
        tt.set_default_formatter(&tinytemplate::format_unescaped);
        tt.add_template("page", PAGE_TEMPLATE)?;
        tt.add_template("card", CARD_TEMPLATE)?;

        // Render cards.
        let mut cards = String::new();
        for card in self.cards {
            cards.push_str(&tt.render("card", &card)?);
        }

        // Render page.
        let page = tt.render(
            "page",
            &Page {
                title: self.title,
                style: STYLE_TEMPLATE.to_owned(),
                cards,
            },
        )?;

        Ok(page)
    }
}
