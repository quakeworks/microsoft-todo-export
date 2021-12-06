#[macro_use]
extern crate serde;
#[macro_use]
extern crate derive_more;

use std::{fs, io};
use std::collections::HashMap;
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
mod onenote;
mod todo;

use error::Result;
use quake_microsoft_todo::Collection;
use quake_microsoft_todo::tasks::{TodoTask, WellknownListName};
use crate::onenote::download_page;

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
    pub lastModifiedDateTime: String,
    pub title: String,
    pub contentUrl: String,
}

impl Default for PageVO {
    fn default() -> Self {
        PageVO {
            sourceUrl: "".to_string(),
            id: "".to_string(),
            createdDateTime: "".to_string(),
            lastModifiedDateTime: "".to_string(),
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

    onenote::dump_onenotes(token);

    // let user_id = "";
    // let client = Graph::new(token);
    // download_pages(&client, user_id);

    Ok(())
}

fn download_pages(client: &Graph<BlockingHttpClient>, user_id: &str) {
    let content = fs::read_to_string("sections-output.json").unwrap();
    let sections: Vec<SectionVO> = serde_json::from_str(&content).unwrap();

    let mut urls: Vec<String> = vec![];
    let mut id_url_map: HashMap<String, String> = HashMap::new();
    for section in &sections {
        for page in &section.pages {
            id_url_map.insert(page.id.clone(), page.contentUrl.clone());
            urls.push(page.contentUrl.clone());
        }
    }

    fs::write("urls", urls.join("\n")).unwrap();

    let mut fails = download(&client, &mut id_url_map, user_id);

    while !fails.is_empty() {
        fails = download(&client, &mut id_url_map, user_id);
    }
}

fn download(client: &Graph<BlockingHttpClient>, id_url_map: &mut HashMap<String, String>, user_id: &str) -> HashMap<String, String> {
    let mut fails: HashMap<String, String> = HashMap::new();
    for (id, url) in id_url_map {
        println!("downloading {:?}", url);
        if download_page(&client, user_id, id.as_str()).is_err() {
            fails.insert(id.to_string(), url.to_string());
        }
    }
    fails
}
