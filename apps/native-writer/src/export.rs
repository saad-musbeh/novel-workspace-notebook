use novel_storage::ProjectBundle;
fn escape(text:&str)->String{
    text.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;")
}
pub fn manuscript(bundle:&ProjectBundle,format:&str)->String{
    let mut output=if format=="html"{
        format!("<!doctype html><html><meta charset=\"utf-8\"><title>{}</title><body><h1>{}</h1>",escape(&bundle.project.title),escape(&bundle.project.title))
    }else{
        format!("{}{}\n\n",if format=="markdown"{
            "# "
        }else{
            ""
        },bundle.project.title)
    };
    for chapter in &bundle.chapters{
        let title=&chapter.title;
        if format=="html"{
            output+=&format!("<h2>{}</h2>",escape(title));
        }else{
            output+=&format!("{}{}\n\n",if format=="markdown"{
                "## "
            }else{
                ""
            },title);
        }
        for scene in bundle.scenes.iter().filter(|s|s.chapter_id==chapter.id){
            if format=="html"{
                output+=&format!("<h3>{}</h3>",escape(&scene.scene.title));
                for block in scene.scene.document["content"].as_array().into_iter().flatten(){
                    let tag=if block["type"]=="heading"{
                        "h4"
                    }else{
                        "p"
                    };
                    output+=&format!("<{tag} dir=\"{}\" lang=\"{}\">",escape(block["attrs"]["dir"].as_str().unwrap_or("auto")),escape(block["attrs"]["lang"].as_str().unwrap_or("en")));
                    for node in block["content"].as_array().into_iter().flatten(){
                        if node["type"]=="hardBreak"{
                            output+="<br>";
                            continue;
                        }let mut text=escape(node["text"].as_str().unwrap_or(""));
                        for mark in node["marks"].as_array().into_iter().flatten(){
                            let tag=match mark["type"].as_str(){
                                Some("bold")=>"strong",Some("italic")=>"em",Some("underline")=>"u",_=>continue
                            };
                            text=format!("<{tag}>{text}</{tag}>");
                        }output+=&text;
                    }output+=&format!("</{tag}>");
                }
            }else{
                output+=&format!("{}{}\n\n{}\n\n",if format=="markdown"{
                    "### "
                }else{
                    ""
                },scene.scene.title,novel_storage::plain_text(&scene.scene.document));
            }
        }
    }
    if format=="html"{
        output+="</body></html>";
    }output
}
