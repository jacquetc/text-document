//! Tests for AddBlockToList and RemoveBlockFromList use cases.

extern crate text_document_editing as document_editing;
use anyhow::Result;

use document_editing::document_editing_controller;
use document_editing::{AddBlockToListDto, CreateListDto, ListStyle, RemoveBlockFromListDto};

use test_harness::{block_controller, get_block_ids, setup_with_text};

use test_harness::list_controller;

// ═══════════════════════════════════════════════════════════════════
// AddBlockToList tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_add_block_to_list() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("First\nSecond\nThird")?;

    // Create a list on the first block only (position 0..5)
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 0,
            style: ListStyle::Disc,
        },
    )?;
    let list_id = list_result.list_id;

    // The first block should have the list
    let block_ids = get_block_ids(&db)?;
    let first_block = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert_eq!(first_block.list, Some(list_id as u64));

    // The second block should not have a list yet
    let second_block = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert!(second_block.list.is_none());

    // Add the second block to the same list
    document_editing_controller::add_block_to_list(
        &db,
        &hub,
        &mut urm,
        None,
        &AddBlockToListDto {
            block_id: block_ids[1] as i64,
            list_id,
        },
    )?;

    // Verify the second block now has the list
    let second_block = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert_eq!(second_block.list, Some(list_id as u64));

    // Third block should remain without a list
    let third_block = block_controller::get(&db, &block_ids[2])?.unwrap();
    assert!(third_block.list.is_none());

    Ok(())
}

#[test]
fn test_add_block_to_list_undo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("First\nSecond")?;

    // Create a list on first block
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 0,
            style: ListStyle::Decimal,
        },
    )?;

    let block_ids = get_block_ids(&db)?;

    // Add second block to the list
    document_editing_controller::add_block_to_list(
        &db,
        &hub,
        &mut urm,
        None,
        &AddBlockToListDto {
            block_id: block_ids[1] as i64,
            list_id: list_result.list_id,
        },
    )?;

    // Verify second block is in the list
    let second = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert_eq!(second.list, Some(list_result.list_id as u64));

    // Undo
    urm.undo(None)?;

    // Second block should no longer be in the list
    let second = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert!(
        second.list.is_none(),
        "Block should not have a list after undo"
    );

    // Redo
    urm.redo(None)?;

    let second = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert_eq!(
        second.list,
        Some(list_result.list_id as u64),
        "Block should be back in the list after redo"
    );

    Ok(())
}

#[test]
fn test_add_block_to_list_invalid_block() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello")?;

    // Create a list
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 0,
            style: ListStyle::Disc,
        },
    )?;

    // Try adding a non-existent block
    let result = document_editing_controller::add_block_to_list(
        &db,
        &hub,
        &mut urm,
        None,
        &AddBlockToListDto {
            block_id: 99999,
            list_id: list_result.list_id,
        },
    );

    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_add_block_to_list_invalid_list() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello")?;

    let block_ids = get_block_ids(&db)?;

    // Try adding to a non-existent list
    let result = document_editing_controller::add_block_to_list(
        &db,
        &hub,
        &mut urm,
        None,
        &AddBlockToListDto {
            block_id: block_ids[0] as i64,
            list_id: 99999,
        },
    );

    assert!(result.is_err());

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// RemoveBlockFromList tests
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_remove_block_from_list() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("First\nSecond")?;

    // Create a list on both blocks
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 11, // covers both blocks
            style: ListStyle::Disc,
        },
    )?;
    let list_id = list_result.list_id as u64;

    let block_ids = get_block_ids(&db)?;

    // Both blocks should be in the list
    let first = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert_eq!(first.list, Some(list_id));
    let second = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert_eq!(second.list, Some(list_id));

    // Remove the first block from the list
    document_editing_controller::remove_block_from_list(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveBlockFromListDto {
            block_id: block_ids[0] as i64,
        },
    )?;

    // First block should no longer have a list
    let first = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert!(first.list.is_none());

    // Second block should still be in the list
    let second = block_controller::get(&db, &block_ids[1])?.unwrap();
    assert_eq!(second.list, Some(list_id));

    // List should still exist since one block still references it
    let list = list_controller::get(&db, &list_id)?;
    assert!(list.is_some(), "List should still exist");

    Ok(())
}

#[test]
fn test_remove_last_block_from_list_auto_deletes_list() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Solo")?;

    // Create a list on the only block
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 0,
            style: ListStyle::Disc,
        },
    )?;
    let list_id = list_result.list_id as u64;

    let block_ids = get_block_ids(&db)?;

    // Verify the list exists
    assert!(list_controller::get(&db, &list_id)?.is_some());

    // Remove the only block from the list — list should be auto-deleted
    document_editing_controller::remove_block_from_list(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveBlockFromListDto {
            block_id: block_ids[0] as i64,
        },
    )?;

    // Block should not have a list
    let block = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert!(block.list.is_none());

    // List should have been auto-deleted
    let list = list_controller::get(&db, &list_id)?;
    assert!(list.is_none(), "List should be auto-deleted when empty");

    Ok(())
}

#[test]
fn test_remove_block_from_list_undo() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("First\nSecond")?;

    // Create a list on both blocks
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 11,
            style: ListStyle::Disc,
        },
    )?;
    let list_id = list_result.list_id as u64;

    let block_ids = get_block_ids(&db)?;

    // Remove first block from list
    document_editing_controller::remove_block_from_list(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveBlockFromListDto {
            block_id: block_ids[0] as i64,
        },
    )?;

    // First block is out
    let first = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert!(first.list.is_none());

    // Undo
    urm.undo(None)?;

    // First block should be back in the list
    let first = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert_eq!(first.list, Some(list_id));

    Ok(())
}

#[test]
fn test_remove_block_from_list_auto_delete_undo_restores_list() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Solo")?;

    // Create a list, then remove the only block — list auto-deleted
    let list_result = document_editing_controller::create_list(
        &db,
        &hub,
        &mut urm,
        None,
        &CreateListDto {
            position: 0,
            anchor: 0,
            style: ListStyle::Disc,
        },
    )?;
    let list_id = list_result.list_id as u64;

    let block_ids = get_block_ids(&db)?;
    document_editing_controller::remove_block_from_list(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveBlockFromListDto {
            block_id: block_ids[0] as i64,
        },
    )?;

    // List is gone
    assert!(list_controller::get(&db, &list_id)?.is_none());

    // Undo — list should be restored
    urm.undo(None)?;

    let list = list_controller::get(&db, &list_id)?;
    assert!(list.is_some(), "List should be restored after undo");

    let block = block_controller::get(&db, &block_ids[0])?.unwrap();
    assert_eq!(block.list, Some(list_id));

    Ok(())
}

#[test]
fn test_remove_block_not_in_list_fails() -> Result<()> {
    let (db, hub, mut urm) = setup_with_text("Hello")?;

    let block_ids = get_block_ids(&db)?;

    // Block is not in a list, should fail
    let result = document_editing_controller::remove_block_from_list(
        &db,
        &hub,
        &mut urm,
        None,
        &RemoveBlockFromListDto {
            block_id: block_ids[0] as i64,
        },
    );

    assert!(result.is_err());

    Ok(())
}
