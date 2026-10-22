// SPDX-License-Identifier: MIT OR Apache-2.0
//! The global workspace (spec sections 12 and 48.1): publish, attend, subscribe, broadcast,
//! request and withdraw.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use tpt_thought_core::{ThoughtContent, Timestamp};

/// Identifier of a workspace item, assigned in publication order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ItemId(pub u64);

/// How an item entered the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    /// Visible on the workspace; not pushed to anyone's inbox.
    Publication,
    /// Visible, and pushed to every subscriber of its topic except the publisher.
    Broadcast,
    /// Asks subscribers of a topic for a response.
    Request,
    /// A reply to a [`ItemKind::Request`], pushed to the requester's inbox.
    Response,
}

/// An item on the workspace.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// Identity.
    pub id: ItemId,
    /// How it entered the workspace.
    pub kind: ItemKind,
    /// Who published it.
    pub publisher: String,
    /// The topic it was published on.
    pub topic: String,
    /// The request this answers, for responses.
    pub in_reply_to: Option<ItemId>,
    /// Payload.
    pub payload: ThoughtContent,
    /// When it was published.
    pub published_at: Timestamp,
}

/// Errors from workspace operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    /// No live item has this identifier (it never existed, or was withdrawn).
    UnknownItem(ItemId),
    /// Only the publisher may withdraw an item.
    NotPublisher(ItemId),
    /// A response was attempted against an item that is not a request.
    NotARequest(ItemId),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownItem(id) => write!(f, "no live workspace item {}", id.0),
            Self::NotPublisher(id) => write!(f, "only the publisher may withdraw item {}", id.0),
            Self::NotARequest(id) => write!(f, "item {} is not a request", id.0),
        }
    }
}

impl std::error::Error for WorkspaceError {}

/// The workspace contract. Anima's shared workspace implements this trait; Thought never
/// depends on the implementation.
pub trait GlobalWorkspace {
    /// Receive future broadcasts and requests on `topic`.
    fn subscribe(&mut self, subscriber: &str, topic: &str);

    /// Stop receiving broadcasts and requests on `topic`. Already-delivered items stay in the
    /// inbox until attended.
    fn unsubscribe(&mut self, subscriber: &str, topic: &str);

    /// Place an item on the workspace without notifying anyone.
    fn publish(
        &mut self,
        publisher: &str,
        topic: &str,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId;

    /// Publish an item and deliver it to every subscriber of `topic` except the publisher.
    fn broadcast(
        &mut self,
        publisher: &str,
        topic: &str,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId;

    /// Ask subscribers of `topic` for a response. Delivered to them as a request; they answer
    /// with [`GlobalWorkspace::respond`].
    fn request(
        &mut self,
        requester: &str,
        topic: &str,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId;

    /// Answer `request`. The response is delivered to the requester's inbox.
    fn respond(
        &mut self,
        responder: &str,
        request: ItemId,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> Result<ItemId, WorkspaceError>;

    /// Attend to a live item: returns it and removes it from `subscriber`'s inbox if present.
    fn attend(&mut self, subscriber: &str, item: ItemId) -> Result<Item, WorkspaceError>;

    /// Identifiers waiting in `subscriber`'s inbox, oldest first.
    fn inbox(&self, subscriber: &str) -> Vec<ItemId>;

    /// Remove an item the caller published. It is removed from every inbox.
    fn withdraw(&mut self, publisher: &str, item: ItemId) -> Result<Item, WorkspaceError>;

    /// Look up a live item without attending to it.
    fn get(&self, item: ItemId) -> Option<&Item>;
}

/// A reference in-process workspace with deterministic ordering throughout.
#[derive(Debug, Clone, Default)]
pub struct InProcessWorkspace {
    next: u64,
    items: BTreeMap<ItemId, Item>,
    subscriptions: BTreeMap<String, BTreeSet<String>>,
    inboxes: BTreeMap<String, Vec<ItemId>>,
}

impl InProcessWorkspace {
    /// An empty workspace.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of live items.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// True if no items are live.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn insert(
        &mut self,
        kind: ItemKind,
        publisher: &str,
        topic: &str,
        in_reply_to: Option<ItemId>,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId {
        let id = ItemId(self.next);
        self.next += 1;
        let item = Item {
            id,
            kind,
            publisher: publisher.to_owned(),
            topic: topic.to_owned(),
            in_reply_to,
            payload,
            published_at: now,
        };
        self.items.insert(id, item);
        id
    }

    fn deliver_to_topic(&mut self, topic: &str, exclude: &str, id: ItemId) {
        let Some(subscribers) = self.subscriptions.get(topic) else {
            return;
        };
        for subscriber in subscribers.iter().filter(|s| s.as_str() != exclude) {
            self.inboxes.entry(subscriber.clone()).or_default().push(id);
        }
    }
}

impl GlobalWorkspace for InProcessWorkspace {
    fn subscribe(&mut self, subscriber: &str, topic: &str) {
        self.subscriptions
            .entry(topic.to_owned())
            .or_default()
            .insert(subscriber.to_owned());
    }

    fn unsubscribe(&mut self, subscriber: &str, topic: &str) {
        if let Some(subscribers) = self.subscriptions.get_mut(topic) {
            subscribers.remove(subscriber);
            if subscribers.is_empty() {
                self.subscriptions.remove(topic);
            }
        }
    }

    fn publish(
        &mut self,
        publisher: &str,
        topic: &str,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId {
        self.insert(ItemKind::Publication, publisher, topic, None, payload, now)
    }

    fn broadcast(
        &mut self,
        publisher: &str,
        topic: &str,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId {
        let id = self.insert(ItemKind::Broadcast, publisher, topic, None, payload, now);
        self.deliver_to_topic(topic, publisher, id);
        id
    }

    fn request(
        &mut self,
        requester: &str,
        topic: &str,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> ItemId {
        let id = self.insert(ItemKind::Request, requester, topic, None, payload, now);
        self.deliver_to_topic(topic, requester, id);
        id
    }

    fn respond(
        &mut self,
        responder: &str,
        request: ItemId,
        payload: ThoughtContent,
        now: Timestamp,
    ) -> Result<ItemId, WorkspaceError> {
        let asked = self
            .items
            .get(&request)
            .ok_or(WorkspaceError::UnknownItem(request))?;
        if asked.kind != ItemKind::Request {
            return Err(WorkspaceError::NotARequest(request));
        }
        let requester = asked.publisher.clone();
        let topic = asked.topic.clone();
        let id = self.insert(
            ItemKind::Response,
            responder,
            &topic,
            Some(request),
            payload,
            now,
        );
        self.inboxes.entry(requester).or_default().push(id);
        Ok(id)
    }

    fn attend(&mut self, subscriber: &str, item: ItemId) -> Result<Item, WorkspaceError> {
        let found = self
            .items
            .get(&item)
            .cloned()
            .ok_or(WorkspaceError::UnknownItem(item))?;
        if let Some(inbox) = self.inboxes.get_mut(subscriber) {
            inbox.retain(|&queued| queued != item);
        }
        Ok(found)
    }

    fn inbox(&self, subscriber: &str) -> Vec<ItemId> {
        self.inboxes.get(subscriber).cloned().unwrap_or_default()
    }

    fn withdraw(&mut self, publisher: &str, item: ItemId) -> Result<Item, WorkspaceError> {
        let owner = &self
            .items
            .get(&item)
            .ok_or(WorkspaceError::UnknownItem(item))?
            .publisher;
        if owner != publisher {
            return Err(WorkspaceError::NotPublisher(item));
        }
        let removed = self.items.remove(&item).expect("checked above");
        for queued in self.inboxes.values_mut() {
            queued.retain(|&q| q != item);
        }
        Ok(removed)
    }

    fn get(&self, item: ItemId) -> Option<&Item> {
        self.items.get(&item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_thought_core::TextContent;

    fn text(s: &str) -> ThoughtContent {
        ThoughtContent::Text(TextContent {
            text: s.into(),
            language: None,
        })
    }

    #[test]
    fn broadcast_reaches_subscribers_but_not_publisher() {
        let mut ws = InProcessWorkspace::new();
        ws.subscribe("reflex", "sensors");
        ws.subscribe("planner", "sensors");
        ws.subscribe("sensor", "sensors");
        ws.subscribe("bystander", "other");
        let id = ws.broadcast("sensor", "sensors", text("hot"), Timestamp(1));
        assert_eq!(ws.inbox("reflex"), vec![id]);
        assert_eq!(ws.inbox("planner"), vec![id]);
        assert!(ws.inbox("sensor").is_empty());
        assert!(ws.inbox("bystander").is_empty());
    }

    #[test]
    fn publish_is_visible_but_not_delivered() {
        let mut ws = InProcessWorkspace::new();
        ws.subscribe("reader", "t");
        let id = ws.publish("writer", "t", text("note"), Timestamp(1));
        assert!(ws.inbox("reader").is_empty());
        assert_eq!(ws.attend("anyone", id).unwrap().payload, text("note"));
    }

    #[test]
    fn unsubscribed_modules_receive_nothing() {
        let mut ws = InProcessWorkspace::new();
        ws.subscribe("a", "t");
        ws.unsubscribe("a", "t");
        ws.broadcast("b", "t", text("x"), Timestamp(1));
        assert!(ws.inbox("a").is_empty());
    }

    #[test]
    fn attend_consumes_inbox_entry_and_returns_item() {
        let mut ws = InProcessWorkspace::new();
        ws.subscribe("a", "t");
        let id = ws.broadcast("b", "t", text("x"), Timestamp(1));
        let item = ws.attend("a", id).unwrap();
        assert_eq!(item.kind, ItemKind::Broadcast);
        assert!(ws.inbox("a").is_empty());
        assert!(ws.get(id).is_some(), "attending does not remove the item");
    }

    #[test]
    fn request_and_response_round_trip() {
        let mut ws = InProcessWorkspace::new();
        ws.subscribe("expert", "help");
        let req = ws.request("asker", "help", text("why?"), Timestamp(1));
        assert_eq!(ws.inbox("expert"), vec![req]);
        let resp = ws
            .respond("expert", req, text("because"), Timestamp(2))
            .unwrap();
        assert_eq!(ws.inbox("asker"), vec![resp]);
        assert_eq!(ws.get(resp).unwrap().in_reply_to, Some(req));
    }

    #[test]
    fn respond_rejects_non_requests_and_unknown_items() {
        let mut ws = InProcessWorkspace::new();
        let p = ws.publish("a", "t", text("x"), Timestamp(1));
        assert_eq!(
            ws.respond("b", p, text("y"), Timestamp(2)),
            Err(WorkspaceError::NotARequest(p))
        );
        assert_eq!(
            ws.respond("b", ItemId(99), text("y"), Timestamp(2)),
            Err(WorkspaceError::UnknownItem(ItemId(99)))
        );
    }

    #[test]
    fn withdraw_removes_item_from_every_inbox() {
        let mut ws = InProcessWorkspace::new();
        ws.subscribe("a", "t");
        ws.subscribe("b", "t");
        let id = ws.broadcast("c", "t", text("x"), Timestamp(1));
        assert_eq!(ws.withdraw("c", id).unwrap().id, id);
        assert!(ws.inbox("a").is_empty());
        assert!(ws.inbox("b").is_empty());
        assert_eq!(ws.attend("a", id), Err(WorkspaceError::UnknownItem(id)));
    }

    #[test]
    fn only_the_publisher_may_withdraw() {
        let mut ws = InProcessWorkspace::new();
        let id = ws.publish("owner", "t", text("x"), Timestamp(1));
        assert_eq!(
            ws.withdraw("intruder", id),
            Err(WorkspaceError::NotPublisher(id))
        );
        assert!(ws.get(id).is_some());
        assert!(ws.withdraw("owner", id).is_ok());
        assert!(ws.is_empty());
    }
}
