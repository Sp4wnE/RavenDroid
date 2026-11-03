//! UI controls retain a 1280x720 logical layout at every selected render size.
pub(crate) fn supported(width:i32,height:i32)->bool {matches!((width,height),(640,360)|(960,540)|(1280,720))}
pub(crate) fn point(x:i32,y:i32,width:i32,height:i32)->(i32,i32) {
    (((x.clamp(0,1279) as i64*width as i64)/1280) as i32,
     ((y.clamp(0,719) as i64*height as i64)/720) as i32)
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn corners_centers_and_limits_match_render_dimensions(){
        for (w,h) in [(640,360),(960,540),(1280,720)] {
            assert!(supported(w,h));assert_eq!(point(640,360,w,h),(w/2,h/2));
            assert_eq!(point(0,0,w,h),(0,0));assert_eq!(point(1280,720,w,h),(w-1,h-1));
            assert_eq!(point(-1,-1,w,h),(0,0));
        }
        assert!(!supported(1280,360));assert!(!supported(0,0));
    }
}
