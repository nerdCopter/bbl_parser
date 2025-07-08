use crate::BBLHeader;

// Count intentionally skipped frames based on log sampling rate
pub fn count_intentionally_skipped_frames(last_iteration: u32, header: &BBLHeader) -> u32 {
    // If no previous frame or invalid iteration, return 0
    if last_iteration == u32::MAX {
        return 0;
    }

    // Limit skipped frames to a reasonable number to prevent overflow
    const MAX_SKIPPED_FRAMES: u32 = 500;

    // Now count frames from last iteration + 1 until we find one that should exist
    let mut count = 0;
    let mut frame_index = last_iteration + 1;

    // Set a limit to prevent infinite loops
    for _ in 0..MAX_SKIPPED_FRAMES {
        if should_have_frame(frame_index, header) {
            break;
        }
        count += 1;
        frame_index += 1;

        // Safety limit
        if count >= MAX_SKIPPED_FRAMES {
            break;
        }
    }

    count
}

// Determine if a frame should exist based on the log sampling rate
pub fn should_have_frame(frame_index: u32, header: &BBLHeader) -> bool {
    // Logic from blackbox_decode.c - shouldHaveFrame()
    // return (frameIndex % log->frameIntervalI + log->frameIntervalPNum - 1) % log->frameIntervalPDenom < log->frameIntervalPNum;

    // Default interval values if not specified in header
    let frame_interval_i = if header.frame_interval_i > 0 {
        header.frame_interval_i
    } else {
        1
    };
    let frame_interval_p_num = if header.frame_interval_p_num > 0 {
        header.frame_interval_p_num
    } else {
        1
    };
    let frame_interval_p_denom = if header.frame_interval_p_denom > 0 {
        header.frame_interval_p_denom
    } else {
        1
    };

    // Use wrapping operations to prevent overflow
    let mod_i = frame_index.wrapping_rem(frame_interval_i);
    let sum = mod_i.wrapping_add(frame_interval_p_num).wrapping_sub(1);
    let mod_denom = sum.wrapping_rem(frame_interval_p_denom);

    mod_denom < frame_interval_p_num
}
