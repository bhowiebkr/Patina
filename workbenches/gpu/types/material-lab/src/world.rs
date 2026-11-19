//! A material target owns its renderer, mesh maps, and kernel engine inside this example.
use crate::{recipe, registry};
use forge_core::AssetInfo;
use forge_render::{Environment, GpuAsset, Light, MeshMaps, OrbitCamera, SceneRenderer};
use patina_gpu::gpu;
use std::sync::{Arc, OnceLock};

pub struct Context {
    pub engine: forge_nodes::Engine,
    pub scene: SceneRenderer,
    pub ball: GpuAsset,
    pub info: AssetInfo,
    pub camera: OrbitCamera,
    pub maps: MeshMaps,
    pub cached_maps: bool,
    light: Light,
}
impl Context {
    pub fn light(&self) -> Light {
        self.light
    }
    pub fn mesh_maps(&self) -> &MeshMaps {
        &self.maps
    }
}
pub struct World(pub Context, pub u32);

pub fn world() -> Result<Arc<World>, String> {
    struct Shared(Result<Arc<World>, String>);
    static WORLD: OnceLock<&'static Shared> = OnceLock::new();
    WORLD
        .get_or_init(|| patina_sdk::shared(|| Shared(open_world().map(Arc::new))))
        .0
        .clone()
}

fn open_world() -> Result<World, String> {
    let gpu = gpu()?;
    if !gpu
        .device
        .features()
        .contains(patina_gpu::wgpu::Features::FLOAT32_FILTERABLE)
        || gpu.device.limits().max_sampled_textures_per_shader_stage < 20
    {
        return Err(
            "Materials need float32 texture filtering and 20 sampled textures per shader stage"
                .into(),
        );
    }
    let session = forge_runtime::Session::new(&gpu.device, &gpu.queue)?;
    let maps = session.mesh_maps().clone();
    let mut light = session.light();
    light.exposure = recipe().manifest.exposure;
    light.env_yaw = recipe().manifest.rotation / 360.0;
    light.env_pitch = recipe().manifest.tilt / 360.0;
    let engine = engine(&maps, &session.info)?;
    Ok(World(
        Context {
            engine,
            scene: session.scene,
            ball: session.ball,
            info: session.info,
            camera: session.camera,
            maps,
            cached_maps: true,
            light,
        },
        crate::SIZE,
    ))
}

fn studio() -> Result<Environment, String> {
    struct Shared(Result<Environment, String>);
    static STUDIO: OnceLock<&'static Shared> = OnceLock::new();
    STUDIO
        .get_or_init(|| {
            patina_sdk::shared(|| {
                Shared((|| {
                    let gpu = gpu()?;
                    let raster = forge_sources::bitmap_bytes(
                        include_bytes!("../../forge-runtime/assets/studio.hdr"),
                        std::path::Path::new("studio.hdr"),
                        4096,
                    )
                    .map_err(|e| e.to_string())?;
                    let forge_sources::Pixels::F32(pixels) = raster.pixels else {
                        return Err("Studio must use HDR pixels".into());
                    };
                    Environment::new(
                        &gpu.device,
                        &gpu.queue,
                        raster.width,
                        raster.height,
                        &pixels,
                    )
                })())
            })
        })
        .0
        .clone()
}

fn engine(maps: &MeshMaps, info: &AssetInfo) -> Result<forge_nodes::Engine, String> {
    let gpu = gpu()?;
    let mut engine = forge_nodes::Engine::with_registry(&gpu.device, registry())?;
    engine.set_mesh_maps(
        &maps.position.view,
        &maps.normal.view,
        &maps.extras.view,
        &maps.view.view,
        maps.size,
        &info.bounds,
    );
    let rows: Vec<_> = info
        .parts
        .iter()
        .map(|p| {
            let [r, g, b, _] = p.material.base_color;
            [r, g, b, 0.0]
        })
        .collect();
    engine.set_part_table(&gpu.device, &rows);
    Ok(engine)
}

/// Use Surface's validated bake cache, or trace its maps at the object's atlas resolution.
pub fn bake(
    object: &crate::object::Geometry,
    size: u32,
    rebake: bool,
) -> Result<Arc<World>, String> {
    if !matches!(size, 128 | 256 | 512 | 1024 | 2048) {
        return Err("Material size must be 128, 256, 512, 1024, or 2048".into());
    }
    crate::object::validate(&object.info, &object.mesh)?;
    let gpu = gpu()?;
    let mut scene = SceneRenderer::new(&gpu.device, &gpu.queue)?;
    scene.set_environment(&gpu.device, Some(&studio()?));
    let info = object.info.clone();
    let ball = GpuAsset::new(&gpu.device, &scene, &info, &object.mesh);
    let cached = if rebake {
        None
    } else {
        cached_maps(object, &ball)?
    };
    let cached_maps = cached.is_some();
    let maps = match cached {
        Some(maps) => maps,
        None => MeshMaps::bake(
            &gpu.device,
            &gpu.queue,
            &ball,
            &info,
            &object.mesh,
            info.atlas_size,
            true,
        )?,
    };
    let engine = engine(&maps, &info)?;
    let camera = OrbitCamera::for_asset(&info);
    let light = Light {
        exposure: recipe().manifest.exposure,
        env_yaw: recipe().manifest.rotation / 360.0,
        env_pitch: recipe().manifest.tilt / 360.0,
        ..Light::for_environment(&info)
    };
    Ok(Arc::new(World(
        Context {
            engine,
            scene,
            ball,
            info,
            camera,
            maps,
            cached_maps,
            light,
        },
        size,
    )))
}

fn cached_maps(
    object: &crate::object::Geometry,
    ball: &GpuAsset,
) -> Result<Option<MeshMaps>, String> {
    use std::io::Read;
    let Some(path) = &object.maps_path else {
        return Ok(None);
    };
    let limit = 16 + 16 * u64::from(object.info.atlas_size).pow(2);
    let mut bytes = Vec::new();
    match std::fs::File::open(path) {
        Ok(file) => {
            file.take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::File::open(path.with_extension("bin.zlib")) {
                Ok(file) => {
                    flate2::read::ZlibDecoder::new(file)
                        .take(limit + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e.to_string()),
            }
        }
        Err(e) => return Err(e.to_string()),
    }
    let gpu = gpu()?;
    MeshMaps::from_bytes(
        &gpu.device,
        &gpu.queue,
        ball,
        &object.info,
        &object.mesh,
        object.info.atlas_size,
        &bytes,
    )
}
