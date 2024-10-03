// Copyright 2023 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::BTreeMap;

use assert_matches2::{assert_let, assert_matches};
use eyeball_im::VectorDiff;
use matrix_sdk::{
    assert_next_matches_with_timeout,
    deserialized_responses::{AlgorithmInfo, EncryptionInfo, VerificationLevel, VerificationState},
};
use matrix_sdk_test::{async_test, sync_timeline_event, ALICE};
use ruma::{
    event_id,
    events::room::{
        encrypted::{
            EncryptedEventScheme, MegolmV1AesSha2ContentInit, Replacement,
            RoomEncryptedEventContent,
        },
        message::{MessageType, RedactedRoomMessageEventContent},
    },
    server_name, EventId,
};
use stream_assert::{assert_next_matches, assert_pending};

use super::TestTimeline;
use crate::timeline::TimelineItemContent;

#[async_test]
async fn test_live_redacted() {
    let timeline = TestTimeline::new();
    let mut stream = timeline.subscribe().await;

    let f = &timeline.factory;

    timeline
        .handle_live_redacted_message_event(*ALICE, RedactedRoomMessageEventContent::new())
        .await;
    let item = assert_next_matches!(stream, VectorDiff::PushBack { value } => value);

    let redacted_event_id = item.as_event().unwrap().event_id().unwrap();

    timeline
        .handle_live_event(
            f.text_msg(" * test")
                .sender(&ALICE)
                .edit(redacted_event_id, MessageType::text_plain("test").into()),
        )
        .await;

    assert_eq!(timeline.controller.items().await.len(), 2);

    let day_divider = assert_next_matches!(stream, VectorDiff::PushFront { value } => value);
    assert!(day_divider.is_day_divider());
}

#[async_test]
async fn test_live_sanitized() {
    let timeline = TestTimeline::new();
    let mut stream = timeline.subscribe().await;

    let f = &timeline.factory;
    timeline
        .handle_live_event(
            f.text_html("**original** message", "<strong>original</strong> message").sender(&ALICE),
        )
        .await;

    let item = assert_next_matches!(stream, VectorDiff::PushBack { value } => value);
    let first_event = item.as_event().unwrap();
    assert_let!(TimelineItemContent::Message(message) = first_event.content());
    assert_let!(MessageType::Text(text) = message.msgtype());
    assert_eq!(text.body, "**original** message");
    assert_eq!(text.formatted.as_ref().unwrap().body, "<strong>original</strong> message");

    let day_divider = assert_next_matches!(stream, VectorDiff::PushFront { value } => value);
    assert!(day_divider.is_day_divider());

    let first_event_id = first_event.event_id().unwrap();

    let new_plain_content = "!!edited!! **better** message";
    let new_html_content = "<edited/> <strong>better</strong> message";
    timeline
        .handle_live_event(
            f.text_html(format!("* {}", new_plain_content), format!("* {}", new_html_content))
                .sender(&ALICE)
                .edit(
                    first_event_id,
                    MessageType::text_html(new_plain_content, new_html_content).into(),
                ),
        )
        .await;

    let item = assert_next_matches!(stream, VectorDiff::Set { index: 1, value } => value);
    let first_event = item.as_event().unwrap();
    assert_let!(TimelineItemContent::Message(message) = first_event.content());
    assert_let!(MessageType::Text(text) = message.msgtype());
    assert_eq!(text.body, new_plain_content);
    assert_eq!(text.formatted.as_ref().unwrap().body, " <strong>better</strong> message");
}

#[async_test]
async fn test_aggregated_sanitized() {
    let timeline = TestTimeline::new();
    let mut stream = timeline.subscribe().await;

    let original_event_id = EventId::new(server_name!("dummy.server"));
    let ev = sync_timeline_event!({
        "content": {
            "formatted_body": "<strong>original</strong> message",
            "format": "org.matrix.custom.html",
            "body": "**original** message",
            "msgtype": "m.text"
        },
        "event_id": &original_event_id,
        "origin_server_ts": timeline.event_builder.next_server_ts(),
        "sender": *ALICE,
        "type": "m.room.message",
        "unsigned": {
            "m.relations": {
                "m.replace": {
                    "content": {
                        "formatted_body": "* <edited/> <strong>better</strong> message",
                        "format": "org.matrix.custom.html",
                        "body": "* !!edited!! **better** message",
                        "m.new_content": {
                            "formatted_body": "<edited/> <strong>better</strong> message",
                            "format": "org.matrix.custom.html",
                            "body": "!!edited!! **better** message",
                            "msgtype": "m.text"
                        },
                        "m.relates_to": {
                            "event_id": original_event_id,
                            "rel_type": "m.replace"
                        },
                        "msgtype": "m.text"
                    },
                    "event_id": EventId::new(server_name!("dummy.server")),
                    "origin_server_ts": timeline.event_builder.next_server_ts(),
                    "sender": *ALICE,
                    "type": "m.room.message",
                }
            }
        }
    });
    timeline.handle_live_event(ev).await;

    let item = assert_next_matches!(stream, VectorDiff::PushBack { value } => value);
    let first_event = item.as_event().unwrap();
    assert_let!(TimelineItemContent::Message(message) = first_event.content());
    assert_let!(MessageType::Text(text) = message.msgtype());
    assert_eq!(text.body, "!!edited!! **better** message");
    assert_eq!(text.formatted.as_ref().unwrap().body, " <strong>better</strong> message");

    let day_divider = assert_next_matches!(stream, VectorDiff::PushFront { value } => value);
    assert!(day_divider.is_day_divider());
}

#[async_test]
async fn test_edit_updates_encryption_info() {
    let timeline = TestTimeline::new();
    let event_factory = &timeline.factory;

    let original_event_id = event_id!("$original_event");

    let mut original_event = event_factory
        .text_msg("**original** message")
        .sender(*ALICE)
        .event_id(original_event_id)
        .into_sync();

    let mut encryption_info = EncryptionInfo {
        sender: (*ALICE).into(),
        sender_device: None,
        algorithm_info: AlgorithmInfo::MegolmV1AesSha2 {
            curve25519_key: "123".to_owned(),
            sender_claimed_keys: BTreeMap::new(),
        },
        verification_state: VerificationState::Verified,
    };

    original_event.encryption_info = Some(encryption_info.clone());

    timeline.handle_live_event(original_event).await;

    let items = timeline.controller.items().await;
    let first_event = items[1].as_event().unwrap();

    assert_eq!(
        first_event.encryption_info().unwrap().verification_state,
        VerificationState::Verified
    );

    assert_let!(TimelineItemContent::Message(message) = first_event.content());
    assert_let!(MessageType::Text(text) = message.msgtype());
    assert_eq!(text.body, "**original** message");

    let mut edit_event = event_factory
        .text_msg(" * !!edited!! **better** message")
        .sender(*ALICE)
        .edit(original_event_id, MessageType::text_plain("!!edited!! **better** message").into())
        .into_sync();
    encryption_info.verification_state =
        VerificationState::Unverified(VerificationLevel::UnverifiedIdentity);
    edit_event.encryption_info = Some(encryption_info);

    timeline.handle_live_event(edit_event).await;

    let items = timeline.controller.items().await;
    let first_event = items[1].as_event().unwrap();

    assert_eq!(
        first_event.encryption_info().unwrap().verification_state,
        VerificationState::Unverified(VerificationLevel::UnverifiedIdentity)
    );

    assert_let!(TimelineItemContent::Message(message) = first_event.content());
    assert_let!(MessageType::Text(text) = message.msgtype());
    assert_eq!(text.body, "!!edited!! **better** message");
}

#[async_test]
async fn test_utd_edit_replaces_original() {
    let timeline = TestTimeline::with_is_room_encrypted(true);

    let mut stream = timeline.subscribe_events().await;

    let f = &timeline.factory;
    let original_event_id = event_id!("$original_event");

    timeline
        .handle_live_event(f.text_msg("original").sender(*ALICE).event_id(original_event_id))
        .await;

    assert_next_matches_with_timeout!(stream, VectorDiff::PushBack { value } => {
        assert_eq!(value.content().as_message().unwrap().body(), "original");
    });

    let encrypted = EncryptedEventScheme::MegolmV1AesSha2(
        MegolmV1AesSha2ContentInit {
            ciphertext: "\
                AwgAEpABqOCAaP6NqXquQcEsrGCVInjRTLHmVH8exqYO0b5Aulhgzqrt6oWVUZCpSRBCnlmvnc96\
                n/wpjlALt6vYUcNr2lMkXpuKuYaQhHx5c4in2OJCkPzGmbpXRRdw6WC25uzzKr5Vi5Fa8B5o1C5E\
                DGgNsJg8jC+cVZbcbVCFisQcLATG8UBDuZUGn3WtVFzw0aHzgxGEc+t4C8J9aWwqwokaEF7fRjTK\
                ma5GZJZKR9KfdmeHR2TsnlnLPiPh5F12hqwd5XaOMQemS2j4pENfxpBlYIy5Wk3FQN0G"
                .to_owned(),
            sender_key: "sKSGv2uD9zUncgL6GiLedvuky3fjVcEz9qVKZkpzN14".to_owned(),
            device_id: "PNQBRWYIJL".into(),
            session_id: "gI3QWFyqg55EDS8d0omSJwDw8ZWBNEGUw8JxoZlzJgU".into(),
        }
        .into(),
    );

    timeline
        .handle_live_event(
            f.event(RoomEncryptedEventContent::new(
                encrypted,
                Some(ruma::events::room::encrypted::Relation::Replacement(Replacement::new(
                    original_event_id.to_owned(),
                ))),
            ))
            .sender(&ALICE),
        )
        .await;

    assert_next_matches_with_timeout!(stream, VectorDiff::Set { index: 0, value } => {
        assert_matches!(value.content(), TimelineItemContent::UnableToDecrypt(..));
    });

    assert_pending!(stream);
}
