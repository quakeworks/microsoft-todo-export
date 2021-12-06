use quake_microsoft_todo::Collection;
use std::fs;
use crate::{CollectionReader, error, OutputList};

fn dump_todos(token: &str) -> error::Result<()> {
    let client = reqwest::blocking::Client::new();

    let lists: Collection<quake_microsoft_todo::tasks::TodoTaskList> = client.get(crate::graph_url("/me/todo/lists"))
        .bearer_auth(token)
        .send()?
        .json()?;

    let mut output: Vec<OutputList> = vec![];
    for list in lists.value.iter() {
        let fetch_url = crate::graph_url(&format!("/me/todo/lists/{}/tasks", &list.id));

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
