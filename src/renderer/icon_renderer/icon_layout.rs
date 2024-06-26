use crate::renderer::icon_renderer::IconBehavior;
use crate::selection::Bounds;

use super::icon_layout_engine::{create_icon, CrossJustify, Direction, IconLayouts, IconText, Layout, LayoutChild, ScreenLocation, ScreenRelativePosition, ICON_MARGIN, ICON_SIZE };
use super::IconContext;

pub enum IconEvent {
    Copy,
    Screenshot,
    Close,
    ActiveOCRLeft,
    ActiveOCRRight,
    UpdateOCRFormatOption
}

pub fn get_icon_layouts() -> IconLayouts {
    let mut menubar_layout = Layout::new(Direction::Horizontal, CrossJustify::Center, ICON_MARGIN, true);
    menubar_layout.add_icon({
        let mut icon = create_icon!("new-line", IconBehavior::SettingToggle);
        icon.get_active = Some(Box::new(|ctx: &IconContext| { ctx.settings.maintain_newline }));
        icon.click_callback = Some(Box::new(|ctx: &mut IconContext| {
            ctx.settings.maintain_newline = !ctx.settings.maintain_newline;
            ctx.channel.send(IconEvent::UpdateOCRFormatOption).expect("Unable to send update OCR format option event");
        }));
        icon.tooltip_text = Some("Maintain newlines in text (1)".to_string());
        icon
    });
    menubar_layout.add_icon({
        let mut icon = create_icon!("fix-text", IconBehavior::SettingToggle);
        icon.get_active = Some(Box::new(|ctx: &IconContext| { ctx.settings.reformat_and_correct }));
        icon.click_callback = Some(Box::new(|ctx: &mut IconContext| {
            ctx.settings.reformat_and_correct = !ctx.settings.reformat_and_correct;
            ctx.channel.send(IconEvent::UpdateOCRFormatOption).expect("Unable to send update OCR format option event");
        }));
        icon.tooltip_text = Some("Reformat and correct text (2)".to_string());
        icon
    });
    menubar_layout.add_icon({
        let mut icon = create_icon!("settings", IconBehavior::SettingToggle);
        icon.get_active = Some(Box::new(|ctx: &IconContext| { ctx.settings_panel_visible }));
        icon.click_callback = Some(Box::new(|ctx: &mut IconContext| { ctx.settings_panel_visible = !ctx.settings_panel_visible; }));
        icon.tooltip_text = Some("Settings".to_string());
        icon
    });
    menubar_layout.add_icon({
        let mut icon = create_icon!("copy", IconBehavior::Click);
        icon.click_callback = Some(Box::new(|ctx| { ctx.channel.send(IconEvent::Copy).expect("Unable to send copy event"); }));
        icon.get_active = Some(Box::new(|ctx| { ctx.copy_key_held }));
        icon.tooltip_text = Some("Copy (C)".to_string());
        icon.get_disabled = Some(Box::new(|ctx| { !ctx.has_selection }));
        icon
    });
    menubar_layout.add_icon({
        let mut icon = create_icon!("screenshot", IconBehavior::Click);
        icon.click_callback = Some(Box::new(|ctx| { ctx.channel.send(IconEvent::Screenshot).expect("Unable to send screenshot event"); }));
        icon.get_active = Some(Box::new(|ctx| { ctx.screenshot_key_held }));
        icon.tooltip_text = Some("Screenshot selected (S)".to_string());
        icon.get_disabled = Some(Box::new(|ctx| { !ctx.has_selection }));
        icon
    });
    menubar_layout.add_icon({
        let mut icon = create_icon!("close", IconBehavior::Click);
        icon.click_callback = Some(Box::new(|ctx| { ctx.channel.send(IconEvent::Close).expect("Unable to send close event"); }));
        icon.tooltip_text = Some("Close (Esc)".to_string());
        icon
    });

    let mut settings_layout = Layout::new(Direction::Vertical, CrossJustify::Center, ICON_MARGIN * 1.5, false);
    
    macro_rules! horizontal_setting_layout {
        ($name:literal, $icon:literal, $setting:ident) => { horizontal_setting_layout!($name, $icon, $setting, false); };
        ($name:literal, $icon:literal, $setting:ident, $send_format_update:literal) => {
            settings_layout.add_layout({
                let mut layout = Layout::new(Direction::Horizontal, CrossJustify::Center, ICON_MARGIN, true);
                layout.add_text(IconText::new($name.to_string()));
                layout.add_icon({
                    let mut icon = create_icon!($icon, IconBehavior::SettingToggle);
                    icon.get_active = Some(Box::new(|ctx: &IconContext| { ctx.settings.$setting }));
                    icon.click_callback = Some(Box::new(|ctx: &mut IconContext| {
                        ctx.settings.$setting = !ctx.settings.$setting;
                        if $send_format_update {
                            ctx.channel.send(IconEvent::UpdateOCRFormatOption).expect("Unable to send update OCR format option event");
                        }
                    }));
                    icon
                });
                layout
            });
        };
    }

    settings_layout.add_text(IconText::new("Settings".to_string()));
    horizontal_setting_layout!("Maintain newlines in text (1)", "new-line", maintain_newline, true);
    horizontal_setting_layout!("Reformat and correct text (2)", "fix-text", reformat_and_correct, true);
    horizontal_setting_layout!("Background blur enabled (3)", "blur", background_blur_enabled);
    horizontal_setting_layout!("Add pilcrows to preview (4)", "return", add_pilcrow_in_preview);
    horizontal_setting_layout!("Close on copy (5)", "auto-close", close_on_copy);
    horizontal_setting_layout!("Auto copy when selecting (6)", "auto-copy", auto_copy);

    settings_layout.add_layout({
        let mut layout = Layout::new(Direction::Horizontal, CrossJustify::Center, ICON_MARGIN, true);
        layout.add_icon({
            let mut icon = create_icon!("left", IconBehavior::Click);
            icon.click_callback = Some(Box::new(|ctx: &mut IconContext| { ctx.channel.send(IconEvent::ActiveOCRLeft).expect("Unable to send active OCR left event"); }));
            icon
        });
        layout.add_text({
            let mut text = IconText::new("_______________________________________________________".to_string()); // Plenty of characters to make the text allocate enough background tiles
            text.get_text = Some(Box::new(|ctx: &IconContext| { format!("Current OCR: {}", ctx.settings.tesseract_settings.get_ocr_language_data().name) }));
            text
        });
        layout.add_icon({
            let mut icon = create_icon!("right", IconBehavior::Click);
            icon.click_callback = Some(Box::new(|ctx: &mut IconContext| { ctx.channel.send(IconEvent::ActiveOCRRight).expect("Unable to send active OCR right event"); }));
            icon
        });
        layout
    });

    let mut icon_layouts = IconLayouts::new();
    icon_layouts.add_layout(
        String::from("copy"),
        ScreenRelativePosition::new(ScreenLocation::TopLeft, (0., 0.)), // Updated live
        {
            let mut icon = create_icon!("copy", IconBehavior::Click);
            icon.bounds = Bounds::new(0, 0, 25, 25);
            icon.click_callback = Some(Box::new(|ctx| { ctx.channel.send(IconEvent::Copy).expect("Unable to send copy event"); }));
            icon.get_active = Some(Box::new(|ctx| { ctx.copy_key_held }));
            LayoutChild::Icon(icon)
        }
    );
    icon_layouts.add_layout(String::from("menubar"), ScreenRelativePosition::new(ScreenLocation::TopCenter, (0., ICON_SIZE / 2. + ICON_MARGIN)), LayoutChild::Layout(menubar_layout));
    icon_layouts.add_layout(String::from("settings"), ScreenRelativePosition::new(ScreenLocation::TopCenter, (0., ICON_SIZE * 7. + ICON_MARGIN * 2.)), LayoutChild::Layout(settings_layout));

    icon_layouts.initialize();

    return icon_layouts;
}