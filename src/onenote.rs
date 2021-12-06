use std::ffi::OsString;
use std::fs;

use graph_http::{BlockingHttpClient, GraphResponse};
use graph_http::serde_json::Value;
use graph_http::traits::ODataLink;
use graph_rs_sdk::client::Graph;

use crate::{NotebookVO, OnenoteVO, PageVO, SectionVO};

pub fn dump_onenotes(token: &str) {
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
