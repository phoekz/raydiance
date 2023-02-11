pub struct TextBox {
    buffer: String,
}

impl TextBox {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    #[must_use]
    pub fn line<I, A, B>(mut self, kvs: I) -> Self
    where
        I: IntoIterator<Item = (A, B)>,
        A: std::fmt::Display,
        B: std::fmt::Display,
    {
        if !self.buffer.is_empty() {
            self.buffer.push('\n');
        }
        for (i, (k, v)) in kvs.into_iter().enumerate() {
            if i > 0 {
                self.buffer.push_str(", ");
            }
            let k = format!("{k}");
            let v = format!("{v}");
            if v.is_empty() {
                self.buffer.push_str(&k);
            } else {
                self.buffer.push_str(&format!("{k}={v}"));
            }
        }
        self
    }

    #[must_use]
    pub fn build(self) -> String {
        self.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let annotation = TextBox::new();
        assert_eq!(annotation.build(), "");
    }

    #[test]
    fn single() {
        let annotation = TextBox::new().line([("foo", "")]);
        assert_eq!(annotation.build(), "foo");

        let annotation = TextBox::new().line([("foo", "bar")]);
        assert_eq!(annotation.build(), "foo=bar");
    }

    #[test]
    fn multiple() {
        let i = "ba";
        let annotation = TextBox::new().line([("foo", "bar"), ("bar", &format!("{i}z"))]);
        assert_eq!(annotation.build(), "foo=bar, bar=baz");
    }

    #[test]
    fn lines() {
        let annotation = TextBox::new().line([("foo", "bar")]).line([("bar", "baz")]);
        assert_eq!(annotation.build(), "foo=bar\nbar=baz");
    }
}
