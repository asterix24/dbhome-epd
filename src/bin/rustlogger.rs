//% FEATURES: embassy embassy-generic-timers esp-wifi esp-wifi/wifi esp-wifi/utils
//% CHIPS: esp32 esp32s2 esp32s3 esp32c2 esp32c3 esp32c6

#![no_std]
#![no_main]

use core::str::from_utf8;
use embassy_executor::Spawner;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{tcp::TcpSocket, Stack, StackResources};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use embassy_time::{Duration, Timer};

use embedded_io_async::Write;
use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::{
    dma::*,
    dma_buffers,
    prelude::*,
    rng::Rng,
    spi::{
        master::{Config, Spi},
        SpiBitOrder, SpiMode,
    },
    timer::timg::TimerGroup,
};
use esp_println::{print, println};
use esp_wifi::{
    config::PowerSaveMode,
    init,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
        WifiState,
    },
    EspWifiController,
};
use heapless::{String, Vec};

use rustlogger::{
    epd4in2::EPDMgr,
    leds::LedsMgr,
    proto_parser::{reply_err, reply_ok, ParserMgr},
};

// When you are okay with using a nightly compiler it's better to use https://docs.rs/static_cell/2.1.0/static_cell/macro.make_static.html
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

static PROTO_PARSE: Channel<CriticalSectionRawMutex, String<128>, 2> = Channel::new();
static PROTO_RET: Channel<CriticalSectionRawMutex, String<64>, 2> = Channel::new();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = CpuClock::max();
        config
    });

    esp_alloc::heap_allocator!(72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);

    let init = &*mk_static!(
        EspWifiController<'static>,
        init(
            timg0.timer0,
            Rng::new(peripherals.RNG),
            peripherals.RADIO_CLK,
        )
        .unwrap()
    );

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice).unwrap();

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timg1.timer0);
    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = 1234; // very random, very secure seed

    // Init network stack
    let stack = &*mk_static!(
        Stack<WifiDevice<'_, WifiStaDevice>>,
        Stack::new(
            wifi_interface,
            config,
            mk_static!(StackResources<3>, StackResources::<3>::new()),
            seed
        )
    );

    let sclk = peripherals.GPIO0;
    let miso = peripherals.GPIO1;
    let mosi = peripherals.GPIO2;
    let cs = peripherals.GPIO9;

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.channel0;
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(32000);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    let spi = Spi::new_with_config(
        peripherals.SPI2,
        Config {
            frequency: 4.MHz(),
            mode: SpiMode::Mode0,
            read_bit_order: SpiBitOrder::MSBFirst,
            write_bit_order: SpiBitOrder::MSBFirst,
        },
    )
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_miso(miso)
    .with_cs(cs)
    .with_dma(dma_channel.configure(false, DmaPriority::Priority0))
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let mut leds = LedsMgr::new(peripherals.GPIO3, peripherals.GPIO4, peripherals.GPIO5);
    let epd = EPDMgr::new(spi, peripherals.GPIO6, peripherals.GPIO7, peripherals.GPIO8);

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(&stack)).ok();
    spawner.spawn(listener_task(&stack)).ok();
    spawner.spawn(epd_task(&stack, epd)).ok();

    //spawner.spawn(getter_task(&stack)).ok();

    let in_chan = PROTO_PARSE.dyn_receiver();
    let out_chan = PROTO_RET.dyn_sender();

    loop {
        let pkg = ParserMgr::new(in_chan.receive().await);
        let reply = match pkg.cmd.as_str() {
            "led" => leds.cmd(pkg),
            //"show" => epd.cmd(pkg).await,
            _ => Err("Invalid Command"),
        };

        let ret = match reply {
            Ok(e) => reply_ok(e),
            Err(e) => reply_err(e),
        };
        out_chan.send(ret).await;
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    let _ = controller.set_power_saving(PowerSaveMode::Maximum);
    println!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}

#[embassy_executor::task]
async fn listener_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut tmp_buffer = [0; 1024];

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    loop {
        let mut socket = TcpSocket::new(&stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(embassy_time::Duration::from_secs(60)));
        if let Err(e) = socket.accept(20000).await {
            println!("accept error: {:?}", e);
            continue;
        }
        loop {
            let n = match socket.read(&mut tmp_buffer).await {
                Ok(0) => {
                    println!("read EOF");
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    println!("read error: {:?}", e);
                    break;
                }
            };

            println!("rxd {}", from_utf8(&tmp_buffer[..n]).unwrap());

            PROTO_PARSE
                .send(String::from_utf8(Vec::from_slice(&tmp_buffer[..n]).unwrap()).unwrap())
                .await;

            let ret_str = PROTO_RET.receive().await;
            if let Err(e) = socket.write_all(ret_str.as_bytes()).await {
                println!("write error: {:?}", e);
                break;
            }
        }
    }
}

#[embassy_executor::task]
async fn epd_task(
    stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>,
    mut epd: EPDMgr<'static>,
) {
    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut tmp_buffer: [u8; 1024] = [0; 1024];
    let mut rx_meta = [PacketMetadata::EMPTY; 10];
    let mut tx_meta = [PacketMetadata::EMPTY; 10];

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    epd.init().await;

    let mut udp_socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    loop {
        udp_socket.bind(23000).unwrap();
        loop {
            match udp_socket.recv_from(&mut tmp_buffer).await {
                Ok((n, sender)) => {
                    let mut field: [u8; 4] = [0; 4];
                    field.copy_from_slice(&tmp_buffer[..4]);
                    let offset = i32::from_ne_bytes(field);
                    field.copy_from_slice(&tmp_buffer[4..8]);
                    let size = u32::from_ne_bytes(field);

                    print!("{}: {:?} {} ", sender, n, offset);

                    if offset < 0 {
                        epd.display_frame().await;
                        continue;
                    }

                    epd.update_frame(&tmp_buffer[8..n], offset as usize, size as usize);
                }
                Err(e) => {
                    println!("UDP Err: {:?}", e);
                    break;
                }
            }
        }
    }
}

//#[embassy_executor::task]
//async fn edp_task(edp: &'static mut EPDMgr<'static>) {
//    println!("edp..");
//    edp.init().await;
//    println!("edp..init");
//
//    edp.display_frame().await;
//
//    //let in_chan = DATA_STREAM.dyn_receiver();
//    loop {
//        println!("edp..");
//        Timer::after(Duration::from_secs(10)).await;
//    }
//}
