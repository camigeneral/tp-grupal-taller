extern crate gtk4;
extern crate relm4;

use self::gtk4::prelude::{OrientableExt, WidgetExt};
use self::relm4::{
    gtk, Component, ComponentController, ComponentParts, ComponentSender, Controller,
    RelmWidgetExt, SimpleComponent,
};

pub struct LoadingModalModel{
    is_visible: bool
}

#[derive(Debug)]
pub enum LoadingModalMsg {
    Show,
    Hide,
}

#[relm4::component(pub)]
impl SimpleComponent for LoadingModalModel {
    type Init = ();
    type Input = LoadingModalMsg;
    type Output = ();

    view! {
        #[name = "overlay"]
        gtk::Overlay {
            #[name = "background"]
            gtk::Box {
                set_hexpand: true,
                set_vexpand: true,
                set_valign: gtk::Align::Fill,
                set_halign: gtk::Align::Fill,
                set_css_classes: &["loading-modal-background"],

                gtk::Box {
                    set_valign: gtk::Align::Center,
                    set_halign: gtk::Align::Center,
                    set_orientation: gtk::Orientation::Vertical,
                    set_spacing: 12,

                    gtk::Spinner {
                        set_spinning: true,
                        set_size_request: (60, 60),
                    },
                    gtk::Label {
                        set_label: "Generando por IA",
                    }
                }
            }
        }
    }

    fn init(
        _init: (),
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = LoadingModalModel { is_visible: true };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, message: Self::Input, _sender: ComponentSender<Self>) {
        match message {
            LoadingModalMsg::Show => self.is_visible = true,
            LoadingModalMsg::Hide => self.is_visible = false,
        }
    }
}