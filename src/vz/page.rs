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
    background-color: #212121;
    color: white;
    margin: 0 auto;
}
.cards {
    display: flex;
    flex-wrap: wrap;
}
.title-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    font-size: 2em;
    padding: 10px;
    width: 400px;
}
.card {
    display: flex;
    flex-direction: column;
    padding: 10px;
}
.card-variant-0 {
    background-color: #323232;
}
.card-variant-1 {
    background-color: #434343;
}
.card-title {
    margin: 4px;
}
"#;

const TITLE_CARD_TEMPLATE: &str = r#"
<div class="title-card card-variant-{variant}">
    <code class="card-title">{group}</code>
</div>
"#;

const CARD_TEMPLATE: &str = r#"
<div class="card card-variant-{variant}">
    <code class="card-title">{name}</code>
    <img src="{image}" alt="{name}" />
</div>
"#;

struct Card {
    group: String,
    name: String,
    image: String,
}

#[derive(Serialize)]
struct PageContext<'a> {
    title: &'a str,
    style: &'a str,
    cards: &'a str,
}

#[derive(Serialize)]
struct TitleCardContext<'a> {
    group: &'a str,
    variant: u32,
}

#[derive(Serialize)]
struct CardContext<'a> {
    name: &'a str,
    image: &'a str,
    variant: u32,
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

    pub fn push_card(&mut self, group: impl ToString, name: impl ToString, image: impl ToString) {
        self.cards.push(Card {
            group: group.to_string(),
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
        tt.add_template("title-card", TITLE_CARD_TEMPLATE)?;
        tt.add_template("card", CARD_TEMPLATE)?;

        // Render cards.
        let mut cards = String::new();
        let mut current_group = None;
        let mut current_variant = 0;
        for card in self.cards {
            if current_group != Some(Cow::Borrowed(card.group.as_str())) {
                current_variant ^= 1;
                cards.push_str(&tt.render(
                    "title-card",
                    &TitleCardContext {
                        group: &card.group,
                        variant: current_variant,
                    },
                )?);
                current_group = Some(Cow::Owned(card.group.clone()));
            }
            cards.push_str(&tt.render(
                "card",
                &CardContext {
                    name: &card.name,
                    image: &card.image,
                    variant: current_variant,
                },
            )?);
        }

        // Render page.
        let page = tt.render(
            "page",
            &PageContext {
                title: self.title.as_str(),
                style: STYLE_TEMPLATE,
                cards: cards.as_str(),
            },
        )?;

        Ok(page)
    }
}
