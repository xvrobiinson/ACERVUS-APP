//! A small Excel (.xlsx) writer for the reports, the same as the web app's
//! (src/lib/xlsx.ts): one sheet per table, a bold title and header row,
//! amounts as real numbers with two decimals, dates as real dates and a
//! bold total row. The workbook is a zip of a few XML files, stored
//! without compression; nothing else is needed.

use chrono::NaiveDate;

use crate::domain::money::Cents;

/// How a column's cells are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    Money,
    Int,
    Date,
}

/// One cell's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Empty,
    Text(String),
    Money(Cents),
    Int(i64),
    Date(NaiveDate),
}

impl From<&str> for Cell {
    fn from(s: &str) -> Self {
        Cell::Text(s.to_string())
    }
}

impl From<String> for Cell {
    fn from(s: String) -> Self {
        Cell::Text(s)
    }
}

pub struct Column {
    pub label: String,
    pub kind: Kind,
    pub width: Option<f32>,
}

pub struct Sheet {
    pub name: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub total: Option<Vec<Cell>>,
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            // Control characters other than tab and newlines aren't allowed in XML.
            c if (c as u32) < 0x20 && !matches!(c, '\t' | '\n' | '\r') => {}
            c => out.push(c),
        }
    }
    out
}

/// Column letters: 0 → A, 26 → AA.
pub fn col_name(i: usize) -> String {
    let mut s = Vec::new();
    let mut n = i + 1;
    while n > 0 {
        s.push(b'A' + ((n - 1) % 26) as u8);
        n = (n - 1) / 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap_or_default()
}

/// Excel's day number for a date (days since 1899-12-30).
pub fn excel_day(d: NaiveDate) -> i64 {
    let base = NaiveDate::from_ymd_opt(1899, 12, 30).unwrap_or_default();
    (d - base).num_days()
}

/// A sheet name Excel accepts: at most 31 characters, none of []:*?/\ and
/// not used yet (case-insensitively).
pub fn sheet_name(name: &str, taken: &mut Vec<String>) -> String {
    let clean: String = name.chars().map(|c| if "[]:*?/\\".contains(c) { ' ' } else { c }).collect();
    let base: String = clean.trim().chars().take(31).collect();
    let base = if base.is_empty() { "Sheet".to_string() } else { base };
    let mut out = base.clone();
    let mut i = 2;
    while taken.contains(&out.to_lowercase()) {
        out = format!("{} {i}", base.chars().take(28).collect::<String>());
        i += 1;
    }
    taken.push(out.to_lowercase());
    out
}

// Styles: 0 plain, 1 bold, 2 money, 3 bold money, 4 date, 5 whole number,
// 6 bold whole number, 7 big bold title.
fn style(kind: Kind, bold: bool) -> u8 {
    match (kind, bold) {
        (Kind::Text, false) => 0,
        (Kind::Text, true) | (Kind::Date, true) => 1,
        (Kind::Money, false) => 2,
        (Kind::Money, true) => 3,
        (Kind::Date, false) => 4,
        (Kind::Int, false) => 5,
        (Kind::Int, true) => 6,
    }
}

fn text_cell(r: &str, v: &str, s: u8) -> String {
    format!(r#"<c r="{r}" t="inlineStr" s="{s}"><is><t xml:space="preserve">{}</t></is></c>"#, esc(v))
}

fn cell(r: &str, v: &Cell, kind: Kind, bold: bool) -> String {
    let s = style(kind, bold);
    match v {
        Cell::Empty => String::new(),
        Cell::Text(t) if t.is_empty() => String::new(),
        Cell::Money(c) => format!(r#"<c r="{r}" s="{s}"><v>{}</v></c>"#, *c as f64 / 100.0),
        Cell::Int(n) => format!(r#"<c r="{r}" s="{s}"><v>{n}</v></c>"#),
        Cell::Date(d) => format!(r#"<c r="{r}" s="{s}"><v>{}</v></c>"#, excel_day(*d)),
        Cell::Text(t) => text_cell(r, t, if bold { 1 } else { 0 }),
    }
}

fn sheet_xml(sh: &Sheet) -> String {
    let mut rows = Vec::new();
    let mut r = 1;
    rows.push(format!(r#"<row r="{r}">{}</row>"#, text_cell(&format!("A{r}"), &sh.title, 7)));
    r += 1;
    if let Some(sub) = &sh.subtitle {
        rows.push(format!(r#"<row r="{r}">{}</row>"#, text_cell(&format!("A{r}"), sub, 0)));
        r += 1;
    }
    r += 1;
    let head = r;
    let line = |r: usize, cells: &[Cell], bold: bool, kinds: &mut dyn Iterator<Item = Kind>| {
        let body: String =
            cells.iter().zip(kinds).enumerate().map(|(i, (v, k))| cell(&format!("{}{r}", col_name(i)), v, k, bold)).collect();
        format!(r#"<row r="{r}">{body}</row>"#)
    };
    let labels: Vec<Cell> = sh.columns.iter().map(|c| Cell::Text(c.label.clone())).collect();
    rows.push(line(r, &labels, true, &mut std::iter::repeat(Kind::Text)));
    r += 1;
    for row in &sh.rows {
        rows.push(line(r, row, false, &mut sh.columns.iter().map(|c| c.kind)));
        r += 1;
    }
    if let Some(total) = &sh.total {
        rows.push(line(r, total, true, &mut sh.columns.iter().map(|c| c.kind)));
    }
    let cols: String = sh
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let w = c.width.unwrap_or(match c.kind {
                Kind::Text => 24.0,
                Kind::Date => 12.0,
                _ => 15.0,
            });
            format!(r#"<col min="{n}" max="{n}" width="{w}" customWidth="1"/>"#, n = i + 1)
        })
        .collect();
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
            r#"<sheetViews><sheetView workbookViewId="0"><pane ySplit="{head}" topLeftCell="A{next}" activePane="bottomLeft" state="frozen"/></sheetView></sheetViews>"#,
            r#"<cols>{cols}</cols><sheetData>{rows}</sheetData></worksheet>"#
        ),
        head = head,
        next = head + 1,
        cols = cols,
        rows = rows.concat()
    )
}

const STYLES: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    r#"<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#,
    r##"<numFmts count="2"><numFmt numFmtId="164" formatCode="#,##0.00"/><numFmt numFmtId="165" formatCode="yyyy-mm-dd"/></numFmts>"##,
    r#"<fonts count="3"><font><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="11"/><name val="Calibri"/></font><font><b/><sz val="14"/><name val="Calibri"/></font></fonts>"#,
    r#"<fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills>"#,
    r#"<borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders>"#,
    r#"<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>"#,
    r#"<cellXfs count="8">"#,
    r#"<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>"#,
    r#"<xf numFmtId="0" fontId="1" fillId="0" borderId="0" xfId="0" applyFont="1"/>"#,
    r#"<xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>"#,
    r#"<xf numFmtId="164" fontId="1" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1"/>"#,
    r#"<xf numFmtId="165" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>"#,
    r#"<xf numFmtId="1" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/>"#,
    r#"<xf numFmtId="1" fontId="1" fillId="0" borderId="0" xfId="0" applyNumberFormat="1" applyFont="1"/>"#,
    r#"<xf numFmtId="0" fontId="2" fillId="0" borderId="0" xfId="0" applyFont="1"/>"#,
    r#"</cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#
);

/// The workbook's bytes.
pub fn workbook(sheets: &[Sheet]) -> Vec<u8> {
    let mut taken = Vec::new();
    let names: Vec<String> = sheets.iter().map(|s| sheet_name(&s.name, &mut taken)).collect();
    let n = sheets.len();
    let mut files: Vec<(String, String)> = vec![
        (
            "[Content_Types].xml".into(),
            format!(
                concat!(
                    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
                    r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#,
                    r#"<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>"#,
                    r#"<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>"#,
                    "{}</Types>"
                ),
                (1..=n)
                    .map(|i| format!(r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
                    .collect::<String>()
            ),
        ),
        (
            "_rels/.rels".into(),
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into(),
        ),
        (
            "xl/workbook.xml".into(),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{}</sheets></workbook>"#,
                names.iter().enumerate().map(|(i, nm)| format!(r#"<sheet name="{}" sheetId="{}" r:id="rId{}"/>"#, esc(nm), i + 1, i + 1)).collect::<String>()
            ),
        ),
        (
            "xl/_rels/workbook.xml.rels".into(),
            format!(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{}<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
                (1..=n)
                    .map(|i| format!(r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>"#))
                    .collect::<String>(),
                n + 1
            ),
        ),
        ("xl/styles.xml".into(), STYLES.into()),
    ];
    for (i, s) in sheets.iter().enumerate() {
        files.push((format!("xl/worksheets/sheet{}.xml", i + 1), sheet_xml(s)));
    }
    zip_stored(&files)
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// A zip with every file stored (no compression), which Excel reads.
fn zip_stored(files: &[(String, String)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, body) in files {
        let (name, data) = (name.as_bytes(), body.as_bytes());
        let crc = crc32(data);
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = Vec::new();
            h.extend_from_slice(&sig.to_le_bytes());
            if central {
                h.extend_from_slice(&20u16.to_le_bytes()); // made by
            }
            h.extend_from_slice(&20u16.to_le_bytes()); // needed to extract
            h.extend_from_slice(&0x0800u16.to_le_bytes()); // names are UTF-8
            h.extend_from_slice(&0u16.to_le_bytes()); // stored
            h.extend_from_slice(&0u16.to_le_bytes()); // time
            h.extend_from_slice(&0x21u16.to_le_bytes()); // date: 1980-01-01
            h.extend_from_slice(&crc.to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes()); // extra
            if central {
                h.extend_from_slice(&0u16.to_le_bytes()); // comment
                h.extend_from_slice(&0u16.to_le_bytes()); // disk
                h.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
                h.extend_from_slice(&0u32.to_le_bytes()); // external attributes
                h.extend_from_slice(&offset.to_le_bytes());
            }
            h.extend_from_slice(name);
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend_from_slice(data);
        central.extend(header(0x0201_4b50, true));
    }
    let (start, size) = (out.len() as u32, central.len() as u32);
    out.extend(central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0, 0, 0, 0]); // disk numbers
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_letters_and_days() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
        assert_eq!(excel_day(NaiveDate::from_ymd_opt(2026, 11, 6).unwrap()), 46332);
    }

    #[test]
    fn sheet_names_are_valid_and_unique() {
        let mut taken = Vec::new();
        assert_eq!(sheet_name("Loans: at the end [month]", &mut taken), "Loans  at the end  month");
        assert_eq!(sheet_name("Summary", &mut taken), "Summary");
        assert_eq!(sheet_name("summary", &mut taken), "summary 2");
        assert_eq!(sheet_name(&"x".repeat(40), &mut taken).chars().count(), 31);
    }

    #[test]
    fn crc_matches_the_standard() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn workbook_is_a_zip_with_every_part() {
        let sheet = Sheet {
            name: "Pagos".into(),
            title: "Reporte".into(),
            subtitle: Some("A & B".into()),
            columns: vec![
                Column { label: "Fecha".into(), kind: Kind::Date, width: None },
                Column { label: "Monto".into(), kind: Kind::Money, width: None },
            ],
            rows: vec![vec![Cell::Date(NaiveDate::from_ymd_opt(2026, 1, 2).unwrap()), Cell::Money(123_456)]],
            total: Some(vec!["Total".into(), Cell::Money(123_456)]),
        };
        let bytes = workbook(&[sheet]);
        assert_eq!(&bytes[..4], b"PK\x03\x04");
        let text = String::from_utf8_lossy(&bytes);
        for part in ["[Content_Types].xml", "xl/workbook.xml", "xl/styles.xml", "xl/worksheets/sheet1.xml"] {
            assert!(text.contains(part), "{part}");
        }
        assert!(text.contains("<v>1234.56</v>"));
        assert!(text.contains("A &amp; B"));
        // The end record counts all six parts.
        let n = bytes.len();
        assert_eq!(&bytes[n - 22..n - 18], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes([bytes[n - 12], bytes[n - 11]]), 6);
    }
}
