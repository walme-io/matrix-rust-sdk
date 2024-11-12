use std::sync::Arc;

use eyeball::{Observable, ObservableWriteGuard, SharedObservable, Subscriber};
use matrix_sdk_base::{deserialized_responses::MemberEvent, RoomMemberships};
use ruma::{
    events::{AnySyncStateEvent, AnySyncTimelineEvent},
    serde::Raw,
    OwnedEventId,
};
use ruma::events::room::member::SyncRoomMemberEvent;
use tokio::sync::Mutex;
use tracing::warn;

use crate::{room::RoomMember, Error, Room};
use crate::event_handler::EventHandlerHandle;

#[derive(Debug, Clone)]
pub struct AskToJoinRequest {
    room: Arc<Room>,
    pub member: RoomMember,
    pub is_unread: bool,
}

impl AskToJoinRequest {
    pub(crate) fn new(room: Arc<Room>, member: &RoomMember, is_unread: bool) -> Self {
        Self { room, member: member.clone(), is_unread }
    }

    pub async fn mark_as_seen(&mut self) -> Result<bool, Error> {
        let Some(event_id) = self.member.event().event_id() else { return Ok(false) };
        self.room.mark_requests_to_join_as_seen(&[event_id.to_owned()]).await?;
        Ok(true)
    }

    pub async fn accept(&self) -> Result<(), Error> {
        self.room.invite_user_by_id(self.member.user_id()).await
    }

    pub async fn decline(&self, reason: Option<&str>) -> Result<(), Error> {
        self.room.kick_user(self.member.user_id(), reason).await
    }

    pub async fn decline_and_ban(&self, reason: Option<&str>) -> Result<(), Error> {
        self.room.ban_user(self.member.user_id(), reason).await
    }
}

pub struct AskToJoinRequestsSubscriber {
    room: Arc<Room>,
}

impl AskToJoinRequestsSubscriber {
    pub fn new(room: Arc<Room>) -> Self {
        Self { room }
    }

    pub async fn subscribe(&self) -> Result<(Subscriber<Vec<AskToJoinRequest>>, EventHandlerHandle), Error> {
        if !self.room.are_members_synced() {
            self.room.sync_members().await?;
        }

        let room = &self.room;

        let requests_to_join = get_requests_to_join(room.clone()).await?;
        let observable = SharedObservable::new(requests_to_join);

        let subscriber = observable.subscribe_reset();

        let handle = self.room.add_event_handler({
            let room = room.clone();
            move |_ev: SyncRoomMemberEvent| async move {
                let Ok(requests_to_join) = get_requests_to_join(room).await else {
                    warn!("Couldn't update members");
                    return;
                };

                warn!("New member state event received. New requests to join: {:?}", requests_to_join.len());

                let mut guard = observable.write();
                ObservableWriteGuard::set(&mut guard, requests_to_join);

                warn!("Updated requests to join");
            }
        });

        Ok((subscriber, handle))
    }

    pub fn unsubscribe(&self, handle: EventHandlerHandle) {
        self.room.remove_event_handler(handle)
    }
}

async fn get_requests_to_join(room: Arc<Room>) -> Result<Vec<AskToJoinRequest>, Error> {
    let knock_members = room.members(RoomMemberships::KNOCK).await?;

    let seen_ids = room.get_seen_requests_to_join().await?;
    let mut requests_to_join = Vec::new();

    for member in knock_members {
        let is_seen = if let Some(event_id) = member.event().event_id() {
            seen_ids.contains(&event_id.to_owned())
        } else {
            false
        };
        let request = AskToJoinRequest::new(room.clone(), &member, !is_seen);
        requests_to_join.push(request);
    }

    Ok(requests_to_join)
}

fn filter_member(event: &MemberEvent, seen_event_ids: &[OwnedEventId]) -> bool {
    if let Some(event_id) = event.event_id() {
        !seen_event_ids.contains(&event_id.to_owned())
    } else {
        true
    }
}
