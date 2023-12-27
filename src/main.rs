use encoding_rs::GBK;
use scraper::Html;
use scraper::Selector;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    if std::env::args().len() != 3 {
        eprintln!(
            "Usage: {} <genre_id> <book_id>",
            std::env::args().nth(0).unwrap()
        );
        return Ok(());
    }
    let url_base: String;
    let url: String;
    if let Some(genre_id) = std::env::args().nth(1) {
        if let Some(book_id) = std::env::args().nth(2) {
            url_base = format!("https://www.piaotia.com/html/{}/{}/", genre_id, book_id);
            url = format!("{}index.html", url_base);
        } else {
            eprintln!(
                "Usage: {} <genre_id> <book_id>",
                std::env::args().nth(0).unwrap()
            );
            return Ok(());
        }
    } else {
        eprintln!(
            "Usage: {} <genre_id> <book_id>",
            std::env::args().nth(0).unwrap()
        );
        return Ok(());
    }

    println!("Fetching {:?}...", url);

    let client = reqwest::Client::new();

    // ---
    // let sema = Arc::new(Semaphore::new(3));
    // let (content, title) = request_url(
    //     sema,
    //     client,
    //     "Test title".into(),
    //     "https://www.piaotia.com/html/15/15278/11209349.html".into(),
    // )
    // .await?;
    // println!("title: {}\n{}", title, content);
    // return Ok(());
    // ---

    let res = reqwest::get(url).await?;
    let bytes = res.bytes().await?;
    let (body, _, _) = GBK.decode(&bytes);

    let document = Html::parse_document(&body);
    let selector = Selector::parse("ul>li>a").unwrap();

    let mut result: Vec<(String, String)> = Vec::new();

    for row in document.select(&selector) {
        result.push((
            row.inner_html(),
            row.value().attr("href").unwrap().to_string(),
        ));
    }

    println!("Length: {}", result.len());

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open("title.txt")
        .expect("Failed to open the file");

    let mut tasks: Vec<tokio::task::JoinHandle<Result<(String, String), reqwest::Error>>> = vec![];
    let sema = Arc::new(Semaphore::new(3));

    for (title, uri) in result {
        let url2 = format!("{}{}", url_base, uri);
        // Spawn a new async task to make the HTTP request
        let task = tokio::spawn(request_url(sema.clone(), client.clone(), title, url2));
        tasks.push(task);
    }

    for task in tasks {
        if let Ok(Ok((content, url))) = task.await {
            println!("Writing to file... {}", url);
            file.write_all(content.as_bytes()).unwrap();
        }
    }

    Ok(())
}

async fn request_url(
    sema: Arc<Semaphore>,
    client: reqwest::Client,
    title: String,
    url: String,
) -> Result<(String, String), reqwest::Error> {
    let _ = sema.acquire().await;

    let res = client.get(&url).send().await?;
    let bytes = res.bytes().await?;
    let (body, _, _) = GBK.decode(&bytes);

    let dom = tl::parse(&body, tl::ParserOptions::default()).unwrap();
    let mut flag = false;
    let mut content = title.clone();
    content.push_str("\n\n");
    println!("{}", &content);
    for child in dom.nodes() {
        if let tl::Node::Raw(b) = child {
            let line = b.as_utf8_str().to_string().replace("&nbsp;", " ");
            if line.starts_with("（快捷键 ←）") {
                break;
            }
            if flag && line.trim().len() > 0 {
                content.push_str(&line);
                content.push_str("\n\n");
            }
            if line.starts_with("返回书页") {
                flag = true;
            }
        }
    }

    println!("{} OK: {}", title, url);
    Ok((content, title))
}
