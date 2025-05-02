use sqlx::PgConnection;

use crate::{database::repository::mods, mod_zip::download_mod, types::{api::ApiError, mod_json::ModJson}};

#[derive(Debug)]
struct ModImport {
    id: u32,
    mod_id: &'static str,
    download_link: &'static str,
}

pub async fn fixeroo(limit_mb: u32, conn: &mut PgConnection) -> Result<(), ApiError> {
    let mod_imports: Vec<ModImport> = vec![
        ModImport {
            id: 3843,
            mod_id: "ninkaz.editor_utils",
            download_link: "https://github.com/flurrybun/ninkaz-editor-utils/releases/download/v1.3.1/ninkaz.editor_utils.geode",
        },
        ModImport {
            id: 3842,
            mod_id: "alphalaneous.improved_group_view",
            download_link: "https://github.com/Alphalaneous/ImprovedGroupView/releases/download/1.0.16/alphalaneous.improved_group_view.geode",
        },
        ModImport {
            id: 3841,
            mod_id: "zilko.level_showcases",
            download_link: "https://github.com/ZiLko/Level-Showcases/releases/download/v1.0.0/zilko.level_showcases.geode",
        },
        ModImport {
            id: 3839,
            mod_id: "ryder7223.autopractice",
            download_link: "https://github.com/ryder7223/Auto-Practice/releases/download/v1.1.7/ryder7223.autopractice.geode",
        },
        ModImport {
            id: 3836,
            mod_id: "raydeeux.revisedlevelcells",
            download_link: "https://github.com/RayDeeUx/RevisedLevelCells/releases/download/v1.2.0/raydeeux.revisedlevelcells.geode",
        },
        ModImport {
            id: 3834,
            mod_id: "alphalaneous.safe_zones_for_ios",
            download_link: "https://github.com/Alphalaneous/Safe-zones-for-iOS/releases/download/1.0.7/alphalaneous.safe_zones_for_ios.geode",
        },
        ModImport {
            id: 3832,
            mod_id: "nwo5.trigger_id_search",
            download_link: "https://github.com/Nwo5-trg/TriggerIDSearch/releases/download/v1.0.0/nwo5.trigger_id_search.geode",
        },
        ModImport {
            id: 3829,
            mod_id: "cvolton.misc_bugfixes",
            download_link: "https://github.com/Cvolton/miscbugfixes-geode/releases/download/v1.6.1/cvolton.misc_bugfixes.geode",
        },
        ModImport {
            id: 3828,
            mod_id: "cvolton.betterinfo",
            download_link: "https://github.com/Cvolton/betterinfo-geode/releases/download/v4.3.10/cvolton.betterinfo.geode",
        },
        ModImport {
            id: 3826,
            mod_id: "syzzi.click_between_frames",
            download_link: "https://github.com/theyareonit/Click-Between-Frames/releases/download/v1.4.5/syzzi.click_between_frames.geode",
        },
        ModImport {
            id: 3825,
            mod_id: "bobby_shmurner.zoom",
            download_link: "https://github.com/BobbyShmurner/Zoom/releases/download/v1.2.4/bobby_shmurner.zoom.geode",
        },
        ModImport {
            id: 3824,
            mod_id: "alphalaneous.asyncweb",
            download_link: "https://github.com/Alphalaneous/AsyncWeb/releases/download/0.1.4/alphalaneous.asyncweb.geode",
        },
        ModImport {
            id: 3823,
            mod_id: "alphalaneous.alphas_geode_utils",
            download_link: "https://github.com/Alphalaneous/Alphas-Geode-Utils/releases/download/1.1.3/alphalaneous.alphas_geode_utils.geode",
        },
        ModImport {
            id: 3821,
            mod_id: "hiimjustin000.more_icons",
            download_link: "https://github.com/hiimjasmine00/MoreIcons/releases/download/v1.12.3/hiimjustin000.more_icons.geode",
        },
        ModImport {
            id: 3819,
            mod_id: "abb2k.custom_icon_size",
            download_link: "https://github.com/abb2k/Custom-icon-size/releases/download/v1.0.6/abb2k.custom_icon_s\nize.geode",
        },
        ModImport {
            id: 3818,
            mod_id: "abb2k.duration_filter",
            download_link: "https://github.com/abb2k/Duration-Filter/releases/download/v1.0.4/abb2k.duration_filter.geode",
        },
        ModImport {
            id: 3817,
            mod_id: "abb2k.demonify",
            download_link: "https://github.com/abb2k/demonify/releases/download/v1.0.4/abb2k.demonify.geode",
        },
        ModImport {
            id: 3816,
            mod_id: "abb2k.gdwt",
            download_link: "https://github.com/abb2k/GDWT/releases/download/GDWT-v1.2.20/abb2k.gdwt.geode",
        },
        ModImport {
            id: 3815,
            mod_id: "elohmrow.death_tracker",
            download_link: "https://github.com/eloh-mrow/death-tracker/releases/download/v2.4.6/elohmrow.death_tracker.geode",
        },
        ModImport {
            id: 3813,
            mod_id: "glow12.groupshift",
            download_link: "https://github.com/glow13/GroupShift/releases/download/v1.1.1/glow12.groupshift.geode",
        },
        ModImport {
            id: 3812,
            mod_id: "prevter.go-indicator",
            download_link: "https://github.com/Prevter/gd-go-indicator/releases/download/v1.2.0/prevter.go-indicator.geode",
        },
        ModImport {
            id: 3811,
            mod_id: "undefined0.rewind",
            download_link: "https://github.com/undefined06855/Rewind/releases/download/v1.1.2/undefined0.rewind.geode",
        },
        ModImport {
            id: 3810,
            mod_id: "hiimjasmine00.smart_bpm_trigger",
            download_link: "https://github.com/hiimjasmine00/SmartBPMTrigger/releases/download/v1.1.4/hiimjasmine00.smart_bpm_trigger.geode",
        },
        ModImport {
            id: 3807,
            mod_id: "ninxout.options_api",
            download_link: "https://github.com/ninXout/OptionsAPI/releases/download/v1.0.1/ninxout.options_api.geode",
        },
        ModImport {
            id: 3805,
            mod_id: "iandyhd3.wsliveeditor",
            download_link: "https://github.com/iAndyHD3/WSLiveEditor/releases/download/v2.4.0/iandyhd3.wsliveeditor.geode",
        },
        ModImport {
            id: 3804,
            mod_id: "zilko.improved_folders",
            download_link: "https://github.com/ZiLko/Improved-Folders/releases/download/v1.0.0/zilko.improved_folders.geode",
        },
        ModImport {
            id: 3802,
            mod_id: "saumondeluxe.rainbow_icon",
            download_link: "https://github.com/shadowforce78/Rainbow-Icon/releases/download/1.3.1/saumondeluxe.rainbow_icon.geode",
        },
        ModImport {
            id: 3801,
            mod_id: "techstudent10.gdguesser",
            download_link: "https://github.com/TechStudent10/GDGuesser/releases/download/v1.0.0-beta.8/techstudent10.gdguesser.geode",
        },
        ModImport {
            id: 3800,
            mod_id: "timestepyt.deltarune_textboxes",
            download_link: "https://github.com/TimeStepYT/DeltaruneTextboxes/releases/download/v1.4.3/timestepyt.deltarune_textboxes.geode",
        },
        ModImport {
            id: 3795,
            mod_id: "n.friends",
            download_link: "https://github.com/NicknameGG/friends-/releases/download/1.1.2/n.friends.geode",
        },
        ModImport {
            id: 3793,
            mod_id: "freakyrobot.deathmarkers",
            download_link: "https://github.com/MaSp005/deathmarkers/releases/download/v1.1.0/freakyrobot.deathmarkers.geode",
        },
        ModImport {
            id: 3790,
            mod_id: "tobyadd.gdh",
            download_link: "https://github.com/TobyAdd/GDH/releases/download/v5.0.0-beta.5/tobyadd.gdh.geode",
        },
    ];

    for i in mod_imports {
        let bytes = download_mod(i.download_link, limit_mb).await?;
        let json = ModJson::from_zip(bytes, i.download_link, true)?;

        mods::update_with_json_fixeroo(i.mod_id, &json, &mut *conn).await?;
    }

    Ok(())
}
