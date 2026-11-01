//! Diagnostic-only RGB PNG encoding. Stored DEFLATE avoids new dependencies.
fn crc(bytes:&[u8])->u32{
    let mut value=!0u32;
    for &byte in bytes{value^=byte as u32;for _ in 0..8{value=(value>>1)^(0xedb88320u32.wrapping_mul(value&1));}}
    !value
}
fn chunk(output:&mut Vec<u8>,kind:&[u8;4],bytes:&[u8]){
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    let start=output.len();output.extend_from_slice(kind);output.extend_from_slice(bytes);
    let checksum=crc(&output[start..]);output.extend_from_slice(&checksum.to_be_bytes());
}
pub(super) fn encode(width:u32,height:u32,rgb:&[u8])->anyhow::Result<Vec<u8>>{
    let row=(width as usize).checked_mul(3).ok_or_else(||anyhow::anyhow!("PNG row overflow"))?;
    anyhow::ensure!(width>0&&height>0&&row.checked_mul(height as usize)==Some(rgb.len()),"Invalid RGB PNG dimensions");
    let mut raw=Vec::with_capacity(rgb.len()+height as usize);
    for bytes in rgb.chunks_exact(row){raw.push(0);raw.extend_from_slice(bytes);}
    let mut zlib=vec![0x78,0x01];
    let blocks=raw.len().div_ceil(65535);
    for (index,bytes) in raw.chunks(65535).enumerate(){
        zlib.push((index+1==blocks) as u8);let length=bytes.len() as u16;
        zlib.extend_from_slice(&length.to_le_bytes());zlib.extend_from_slice(&(!length).to_le_bytes());zlib.extend_from_slice(bytes);
    }
    let(mut a,mut b)=(1u32,0u32);
    for &byte in &raw{a=(a+byte as u32)%65521;b=(b+a)%65521;}
    zlib.extend_from_slice(&((b<<16)|a).to_be_bytes());
    let mut output=b"\x89PNG\r\n\x1a\n".to_vec();let mut header=Vec::new();
    header.extend_from_slice(&width.to_be_bytes());header.extend_from_slice(&height.to_be_bytes());header.extend_from_slice(&[8,2,0,0,0]);
    chunk(&mut output,b"IHDR",&header);chunk(&mut output,b"IDAT",&zlib);chunk(&mut output,b"IEND",&[]);Ok(output)
}
#[cfg(test)]mod tests{
    #[test]fn crc_reference_and_invalid_dimensions(){
        assert_eq!(super::crc(b"123456789"),0xcbf43926);
        assert!(super::encode(0,1,&[]).is_err());assert!(super::encode(2,1,&[1,2,3]).is_err());
        assert!(super::encode(1,1,&[255,0,127]).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}
