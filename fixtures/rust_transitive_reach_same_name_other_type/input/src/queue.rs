use crate::Site;

pub(crate) struct Queue {
    jobs: Vec<String>,
}

impl Queue {
    pub(crate) fn full_build(site: &Site) -> Queue {
        let mut queue = Queue { jobs: Vec::new() };
        for lang in &site.langs {
            if lang.feeds == false {
                continue;
            }
            queue.jobs.push(format!("{}/atom.xml", lang.code));
        }
        queue
    }

    pub(crate) fn outputs(&self) -> Vec<String> {
        self.jobs.clone()
    }
}
