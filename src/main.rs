use std::{env, sync::{Mutex, Arc}, ops::Deref, thread::{self, JoinHandle}, time::{Instant}, num::NonZeroU32};
use image::DynamicImage;
use walkdir::WalkDir;

use fast_image_resize as fr;

// @TODO: Get path from input params or user input, go to directory, get all images inside, start resizing and
// greyscaling images using opencv (unless image was already checked and is in database),
// if image was processed send to GPU to process hash.

//After all processing check for hashes that are too similar (identical hashes mean identical images,
// between 1 and 10 similar bits, images are close together, 11+ images are different, will probably
// end up creating a percentage based on similar bits)

fn process_image(image: DynamicImage, image_vector: Arc<Mutex<Vec<i64>>>) {
    // Tested with GPU, takes too long to get kernel ready currently
    // So just forcefully convert into Luma 8
    let grayscale = image.grayscale();

    let og_width = NonZeroU32::new(grayscale.width()).unwrap();
    let og_height = NonZeroU32::new(grayscale.height()).unwrap();
    
    let src_image = fr::Image::from_vec_u8(og_width, og_height, grayscale.to_luma8().into_raw(), fr::PixelType::U8).unwrap();

    let dst_width = NonZeroU32::new(8).unwrap();
    let dst_height = NonZeroU32::new(9).unwrap();
    let mut dst_image = fr::Image::new(dst_width, dst_height, src_image.pixel_type());
    
    // Get mutable view of destination image data
    let mut dst_view = dst_image.view_mut();

    // Create Resizer instance and resize source image
    // into buffer of destination image
    let mut resizer = fr::Resizer::new(
        fr::ResizeAlg::Convolution(fr::FilterType::Lanczos3),
    );
    unsafe {
        // Set CPU extension to default, will automatically select best CPU instruction set
        // TODO: Work on GPU resize in case CPU isn't compatible (and GPU also exists)
        resizer.set_cpu_extensions(fr::CpuExtensions::default());
    }
    resizer.resize(&src_image.view(), &mut dst_view).unwrap();

    // let mut result_buf = BufWriter::new(Vec::new());
    // JpegEncoder::new(&mut result_buf)
    //     .write_image(
    //         dst_image.buffer(),
    //         dst_width.get(),
    //         dst_height.get(),
    //         ColorType::L8,
    //     )
    //     .unwrap();

    let mut result_bytes: i64 = 0;
    let mut current_mask: i64 = 0000000000000001;

    for i in 0..8{
        for j in 0..8{
            let j_val = j*8;
            let j_val_check = (j+1)*8;
            let search_val = j_val + i;
            let search_val_check = j_val_check + i;
            let p1 = dst_image.buffer()[search_val];
            let p2 = dst_image.buffer()[search_val_check];

            if p2 > p1 {
                //we write 1
                result_bytes |= current_mask;
            }else {
                // we write a 0 (or just don't do shit, also works)
            }
            current_mask = current_mask << 1;
        }
    }

    let mut vector = image_vector.lock().unwrap();
    vector.push(result_bytes);

}

fn main() {

    let args: Vec<String> = env::args().collect();

    let path_given = &args[1];
    let image_vector: Arc<Mutex<Vec<i64>>> = Arc::new(Mutex::new(vec![]));
    let mut thread_handlers: Vec<JoinHandle<()>> = vec![];

    let time_before = Instant::now();

    for entry in WalkDir::new(path_given.as_str()).into_iter().filter_map(|e| e.ok()) {
        let file_type = entry.file_type();
        // Now let's show our entry's file type!
        if file_type.is_file() {
            let image = image::open(entry.path());
            
            if let Ok(image) = image {
                // Start processing image
                let vector_clone = Arc::clone(&image_vector);
                let handler = thread::spawn(|| 
                    process_image(image, vector_clone));

                thread_handlers.push(handler);
                
            }
            
        }
    }

    for handler in thread_handlers {
        let _result = handler.join();
    }

    let shit = image_vector.deref().lock().unwrap();
    
    for i in 0..shit.len() {
        if i + 1 < shit.len() {
            let diff = shit[i] ^ shit[i+1];
            if diff == 0{
                println!("Images {} and {} are identical", shit[i], shit[i+1]);
            }else if diff.count_ones() < 5 {
                println!("Images {} and {} are very similar", shit[i], shit[i+1]);
            }else if diff.count_ones() >= 5 && diff.count_ones() < 10 {
                println!("Images {} and {} are suspiciously similar", shit[i], shit[i+1]);
            }else{
                println!("Images {} and {} have nothing in common m'dude", shit[i], shit[i+1]);
            }
        } 
    }

    let time_total = time_before.elapsed();
    println!("Time to calculate stuff: {}ms", time_total.as_millis());

}
