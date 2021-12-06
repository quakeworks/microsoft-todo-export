#[macro_use]
extern crate serde;
#[macro_use]
extern crate derive_more;

use std::{fs, io};
use std::ffi::OsString;
use std::path::PathBuf;
use graph_http::{BlockingDownloadError, BlockingHttpClient};
use graph_http::serde_json::Map;
use graph_http::traits::ODataLink;
use graph_rs_sdk::client::Graph;
use graph_rs_sdk::prelude::GraphResponse;
use graph_rs_sdk::serde_json::Value;
use serde::de::DeserializeOwned;

mod error;
mod slug;

use error::Result;
use quake_microsoft_todo::Collection;
use quake_microsoft_todo::tasks::{TodoTask, WellknownListName};
use crate::slug::slugify;

const GRAPH_BASE_URI: &str = "https://graph.microsoft.com/beta";

/// A hacky fucking struct for reading from a paged `Collection`.
struct CollectionReader<'a, T> where T: DeserializeOwned + Clone {
    /// The `reqwest` client from which to read the next links (pages) in the collection.
    client: &'a reqwest::blocking::Client,

    /// The user's OAuth access `token`.
    token: &'a str,

    /// The `Collection` to read.
    collection: Option<Collection<T>>,

    /// The full list of `items` which have been read from the `Collection` so far.
    items: Vec<T>,

    /// When iterating, the current index in the `items` vec.
    iter_index: usize,
}

impl<'a, T: DeserializeOwned + Clone> CollectionReader<'a, T> {
    /// Create a new collection reader, given the `client` and the access `token`.
    pub fn new(client: &'a reqwest::blocking::Client, token: &'a str) -> Self {
        Self {
            client,
            token,
            collection: None,
            items: Vec::new(),
            iter_index: 0,
        }
    }

    /// First action:
    /// Fetch the requested collection from the given `url`.
    pub fn fetch<S: AsRef<str>>(&mut self, url: S) -> Result<usize> {
        self.fetch_inner(url)
    }

    /// Fetch the requested collection from the given `url`.
    /// Append the received collection items into the `items` property.
    fn fetch_inner<S: AsRef<str>>(&mut self, url: S) -> Result<usize> {
        self.collection = Some(self.client
            .get(url.as_ref())
            .bearer_auth(self.token)
            .send()?
            .json()?);

        let new_item_count = self.collection.as_ref().unwrap().value.len();

        // Copy all of the newly fetched items and put them into the `items` vec.
        self.items.append(&mut self.collection.as_ref().unwrap().value.clone());

        Ok(new_item_count)
    }

    /// Fetch the next page of items into the `items` property.
    pub fn fetch_next(&mut self) -> Result<usize> {
        if self.collection.is_none() {
            return Ok(0);
        }

        let link = self.collection.as_ref().unwrap().odata.next_link.clone();
        println!("{:?}", link);

        match link {
            Some(link) => self.fetch_inner(&link),
            None => Ok(0)
        }
    }

    /// Does the collection have any further links (pages)?
    pub fn has_next_link(&self) -> bool {
        match &self.collection {
            Some(c) => c.odata.next_link.is_some(),
            None => false
        }
    }
}

impl<'a, T: DeserializeOwned + Clone> Iterator for CollectionReader<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.collection.is_none() {
            return None;
        }

        if self.collection.as_ref().unwrap().value.is_empty() {
            return None;
        }

        // If we're at the last item in the currently loaded items
        if self.iter_index == self.items.len() {
            if !self.has_next_link() {
                return None;
            }

            self.fetch_next().expect("Failed to fetch next items!");
        }

        if self.iter_index == self.items.len() {
            return None;
        }

        let fetch_index = self.iter_index;
        self.iter_index += 1;

        self.items.get(fetch_index).map(|item_ref| item_ref.clone())
    }
}


fn graph_url(path: &str) -> String {
    format!("{}{}", GRAPH_BASE_URI, path)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputList {
    pub display_name: String,
    pub id: String,
    pub wellknown_list_name: WellknownListName,
    pub children: Vec<TodoTask>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OnenoteVO {
    pub notebooks: Vec<NotebookVO>,
}

impl Default for OnenoteVO {
    fn default() -> Self {
        OnenoteVO {
            notebooks: vec![]
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NotebookVO {
    pub sourceUrl: String,
    pub id: String,
    pub createdDateTime: String,
    pub displayName: String,
    pub lastModifiedDateTime: String,
    pub sections: Vec<SectionVO>,
}

impl Default for NotebookVO {
    fn default() -> Self {
        NotebookVO {
            sourceUrl: "".to_string(),
            id: "".to_string(),
            createdDateTime: "".to_string(),
            displayName: "".to_string(),
            lastModifiedDateTime: "".to_string(),
            sections: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SectionVO {
    pub sourceUrl: String,
    pub id: String,
    pub createdDateTime: String,
    pub displayName: String,
    pub lastModifiedDateTime: String,
    pub parentName: String,
    pub pages: Vec<PageVO>,
}

impl Default for SectionVO {
    fn default() -> Self {
        SectionVO {
            sourceUrl: "".to_string(),
            id: "".to_string(),
            createdDateTime: "".to_string(),
            displayName: "".to_string(),
            lastModifiedDateTime: "".to_string(),
            parentName: "".to_string(),
            pages: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PageVO {
    pub sourceUrl: String,
    pub id: String,
    pub createdDateTime: String,
    pub title: String,
    pub contentUrl: String,
}

impl Default for PageVO {
    fn default() -> Self {
        PageVO {
            sourceUrl: "".to_string(),
            id: "".to_string(),
            createdDateTime: "".to_string(),
            title: "".to_string(),
            contentUrl: "".to_string(),
        }
    }
}


fn main() -> Result<()> {
    // To acquire OAuth token, grant all "Tasks" permissions within MS Graph Explorer, then click "Access Token"
    // See: https://blog.osull.com/2020/09/14/backup-migrate-microsoft-to-do-tasks-with-powershell-and-microsoft-graph/
    // See: https://gotoguy.blog/2020/05/06/oauth-authentication-to-microsoft-graph-with-powershell-core/
    println!("Paste OAuth2 Token");

    let mut token = String::new();
    io::stdin().read_line(&mut token).expect("Failed to read line");
    let token = token.trim();

    // dump_todos(token);

    dump_onenotes(token);

    Ok(())
}

fn dump_onenotes(token: &str) {
    let client = Graph::new(token);
    let user_id: &str = "";

    // let (urls, onenote_vo) = download_sections_from_top(&client, user_id);

    let get_sections = client
        .v1()
        .user(user_id)
        .onenote()
        .list_sections()
        .send();

    let mut urls = vec![];
    let mut all_sections = vec![];
    let mut page_index = 1;
    match get_sections {
        Ok(section) => {
            let mut sections = build_sections(&client, user_id, &mut urls, section, &mut page_index);
            all_sections.append(&mut sections);
        }
        Err(err) => {
            println!("{:?}", err);
        }
    }

    let string = serde_json::to_string(&all_sections).unwrap();
    fs::write("sections-output.json", string).unwrap();

    fs::write("urls", urls.join("\n")).unwrap();
}

fn download_sections_from_top(client: &Graph<BlockingHttpClient>, user_id: &str) {
    let notebooks = client
        .v1()
        .user(user_id)
        .onenote()
        .notebooks()
        .list_notebooks()
        .send();

    let mut urls: Vec<String> = vec![];

    let mut page_index: usize = 1;
    let mut onenote_vo = OnenoteVO::default();
    match notebooks {
        Ok(notebook) => {
            let vec = notebook.body()["value"].as_array().unwrap();

            for value in vec.iter() {
                let notebook_id = value["id"].as_str().unwrap();
                let book_name = value["displayName"].as_str().unwrap();

                let mut notebook_vo = NotebookVO {
                    sourceUrl: notebook.url().to_string(),
                    id: notebook_id.to_string(),
                    createdDateTime: value["createdDateTime"].as_str().unwrap().to_string(),
                    displayName: book_name.to_string(),
                    lastModifiedDateTime: value["lastModifiedDateTime"].as_str().unwrap().to_string(),
                    sections: vec![],
                };

                println!("bookName: {:}", book_name.to_string());

                let get_sections = client
                    .v1()
                    .user(user_id)
                    .onenote()
                    .notebook(notebook_id)
                    .list_sections()
                    .send();

                match get_sections {
                    Ok(section) => {
                        let mut sections = build_sections(&client, user_id, &mut urls, section, &mut page_index);
                        notebook_vo.sections.append(&mut sections);
                    }
                    Err(err) => {
                        println!("{:?}", err);
                    }
                }

                let mut section_groups_sections = fetch_sections_group_sections(&client, user_id, &mut urls, &mut page_index, notebook_id);

                notebook_vo.sections.append(&mut section_groups_sections);
                onenote_vo.notebooks.push(notebook_vo);
            }
        }
        Err(err) => {
            println!("{:?}", err);
        }
    }

    let string = serde_json::to_string(&onenote_vo).unwrap();
    fs::write("onenote-output.json", string).unwrap();

    fs::write("urls", urls.join("\n")).unwrap();
}

fn fetch_sections_group_sections(client: &Graph<BlockingHttpClient>, user_id: &str, mut urls: &mut Vec<String>, mut page_index: &mut usize, notebook_id: &str) -> Vec<SectionVO> {
    let mut section_groups_sections = vec![];
    let get_section_groups = client
        .v1()
        .user(user_id)
        .onenote()
        .notebook(notebook_id)
        .list_section_groups()
        .send();

    match get_section_groups {
        Ok(section) => {
            let vec = section.body()["value"].as_array().unwrap();
            for value in vec.iter() {
                let section_group_id = value["id"].as_str().unwrap();

                let get_sections = client
                    .v1()
                    .user(user_id)
                    .onenote()
                    .section_group(section_group_id)
                    .list_sections()
                    .send();

                match get_sections {
                    Ok(section) => {
                        let mut sections = build_sections(&client, user_id, &mut urls, section, &mut page_index);
                        section_groups_sections.append(&mut sections);
                    }
                    Err(err) => {
                        println!("{:?}", err);
                    }
                }
            }
        }
        Err(err) => {
            println!("{:?}", err);
        }
    }
    section_groups_sections
}

fn build_sections(client: &Graph<BlockingHttpClient>, user_id: &str, urls: &mut Vec<String>, section: GraphResponse<Value>, index: &mut usize) -> Vec<SectionVO> {
    let vec = section.body()["value"].as_array().unwrap();
    let mut sections: Vec<SectionVO> = vec![];
    for value in vec.iter() {
        let section_id = value["id"].as_str().unwrap();
        let section_name = value["displayName"].as_str().unwrap();

        if !(section_name == "文章") {
            continue
        }

        println!("    sections name: {:}", section_name);

        let parentName = match value["parentSection"].as_object() {
            None => {
                "".to_string()
            }
            Some(obj) => {
                match obj.get("displayName") {
                    None => {
                        "".to_string()
                    }
                    Some(name) => {
                        name.as_str().unwrap().to_string()
                    }
                }
            }
        };

        let mut section_vo = SectionVO {
            sourceUrl: section.url().to_string(),
            id: section_id.to_string(),
            createdDateTime: value["createdDateTime"].as_str().unwrap().to_string(),
            displayName: section_name.to_string(),
            lastModifiedDateTime: value["lastModifiedDateTime"].as_str().unwrap().to_string(),
            parentName: parentName,
            pages: vec![],
        };

        let mut skip: usize = 0;
        let mut pages = fetch_pages(client, user_id, urls, index, section_id, &mut skip);

        section_vo.pages.append(&mut pages);

        println!("section's pages len: {:}", section_vo.pages.len());
        sections.push(section_vo);
    }

    sections
}

fn fetch_pages(client: &Graph<BlockingHttpClient>, user_id: &str, urls: &mut Vec<String>, index: &mut usize, section_id: &str, skip: &mut usize) -> Vec<PageVO> {
    let mut pages = vec![];

    let get_pages = client
        .v1()
        .user(user_id)
        .onenote()
        .section(section_id)
        .list_pages()
        .skip(format!("{:}", skip).as_str())
        .send();

    match get_pages {
        Ok(page) => {
            let vec = page.body()["value"].as_array().unwrap();

            for value in vec.iter() {
                let page_id = value["id"].as_str().unwrap();
                let content_url = value["contentUrl"].as_str().unwrap();

                urls.push(content_url.clone().to_string());

                let title = value["title"].as_str().unwrap().to_string();
                let page_vo = PageVO {
                    sourceUrl: page.url().to_string(),
                    id: value["id"].as_str().unwrap().to_string(),
                    createdDateTime: value["createdDateTime"].as_str().unwrap().to_string(),
                    title: title.clone(),
                    contentUrl: content_url.to_string(),
                };

                // download_page(client, user_id, page_id);

                pages.push(page_vo);
                *index = *index + 1;
            };

            if page.body().next_link().is_some() {
                *skip = *skip + 20;
                let mut new_pages = fetch_pages(client, user_id, urls, index, section_id, skip);
                pages.append(&mut new_pages);
            }
        }
        Err(err) => {
            println!("{:?}", err);
        }
    }

    pages
}

fn download_page(client: &Graph<BlockingHttpClient>, user_id: &str, page_id: &str) {
    let download_page = client
        .v1()
        .user(user_id)
        .onenote()
        .page(page_id)
        .download_page("./content");

    download_page.rename(OsString::from(format!("{:}.html", page_id)));
    let result = download_page.send();

    if let Err(err) = result {
        println!("{:?}", err);
    }
}

fn dump_todos(token: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    let lists: Collection<quake_microsoft_todo::tasks::TodoTaskList> = client.get(graph_url("/me/todo/lists"))
        .bearer_auth(token)
        .send()?
        .json()?;

    let mut output: Vec<OutputList> = vec![];
    for list in lists.value.iter() {
        let fetch_url = graph_url(&format!("/me/todo/lists/{}/tasks", &list.id));

        let mut task_collection = CollectionReader::<quake_microsoft_todo::tasks::TodoTask>::new(&client, &token);
        task_collection.fetch(fetch_url)?;

        let mut list1 = OutputList {
            display_name: list.display_name.to_string(),
            id: list.id.to_string(),
            wellknown_list_name: list.wellknown_list_name.clone(),
            children: task_collection.items.clone(),
        };

        while task_collection.has_next_link() {
            let _ = task_collection.fetch_next();
            list1.children.append(&mut task_collection.items.clone());
        }

        output.push(list1);
    }

    let string = serde_json::to_string(&output).unwrap();
    fs::write("output.json", string).unwrap();

    Ok(())
}

pub fn download(str: &str) {
    // https://github.com/sreeise/graph-rs/blob/e8c9985d986b76f4640bb7825115695ceda6804b/tests/async_drive_request.rs

}